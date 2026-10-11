/// Keeps the source-only parser outside the eager page runtime. The injected text
/// is a trusted SDK asset, not plugin code; merely creating this facade must not
/// compile or execute it. Precompiled modules never need to call parse().
var __operitAcorn = function (parserSource) {
  var parser = null;
  return {
    parse: function (source, options) {
      if (parser === null) {
        // Build into a local export object. A timeout or exception leaves
        // no partially installed globals, so the next invocation can retry.
        var exports = {};
        new Function('exports', 'module', parserSource)(exports, {
          exports: exports
        });
        if (typeof exports.parse !== 'function') throw new Error('Compose parser initialization failed');
        parser = exports;
        // Retain the parser per engine, but release its source after success.
        parserSource = null;
      }
      return parser.parse(source, options);
    }
  };
}(__OPERIT_COMPOSE_PARSER_SOURCE__);
