function isPromise(value) {
  return !!(value && typeof value.then === 'function');
}

/// Keeps navigation queued during intermediate renders until the action completes.
function wrapComposeResponse(bundle, tree, actionResult, includeNavigation, responseSink) {
  var response = {
    navigationCommands: includeNavigation ? bundle.takeNavigationCommands() : []
  };
  if (typeof tree !== 'undefined') {
    response.update = bundle.composition.commit(tree);
  }
  if (typeof actionResult !== 'undefined') {
    response.actionResult = actionResult;
  }
  responseSink.sendComposeResponse(includeNavigation ? 'final' : 'intermediate', response);
  return null;
}

/// Resolves an explicit session sink before any retained commit can be generated.
function requireComposeResponseSink(runtime) {
  if (!runtime || typeof runtime.sendComposeResponse !== 'function') {
    throw new Error('Compose stream command requires its call-owned response sink');
  }
  return runtime;
}

/// Builds a response while preserving the caller's navigation delivery policy.
function buildComposeResponse(bundle, entry, actionResult, includeNavigation, responseSink) {
  var tree = OperitComposeReactive.render(bundle.ctx, entry);
  if (isPromise(tree)) {
    return tree.then(function (resolvedTree) {
      return wrapComposeResponse(bundle, resolvedTree, actionResult, includeNavigation, responseSink);
    });
  }
  return wrapComposeResponse(bundle, tree, actionResult, includeNavigation, responseSink);
}
