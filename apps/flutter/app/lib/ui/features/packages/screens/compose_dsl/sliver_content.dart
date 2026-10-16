// ignore_for_file: file_names

part of '../ToolPkgUiLauncherScreen.dart';

/// Projects session-owned nodes into an external viewport without changing box rendering.
class _ComposeDslSliverContent extends StatefulWidget {
  /// Reuses the session's node handles and dispatchers for each visible row.
  const _ComposeDslSliverContent({required this.renderer});

  final _ComposeDslRenderer renderer;

  /// Creates the observer for the root and its immediate item identities.
  @override
  State<_ComposeDslSliverContent> createState() =>
      _ComposeDslSliverContentState();
}

class _ComposeDslSliverContentState extends State<_ComposeDslSliverContent> {
  final Set<_ComposeDslNode> _dependencies = {};

  /// Reads the live root without reconstructing a snapshot of its descendants.
  _ComposeDslNode get node => widget.renderer.node;

  /// Schedules layout after an observed root or item identity changes.
  void _invalidate() {
    if (mounted) setState(() {});
  }

  /// Releases observers before replacing the root or unmounting the projection.
  void _clearDependencies() {
    for (final dependency in _dependencies) {
      dependency.removeListener(_invalidate);
    }
    _dependencies.clear();
  }

  /// Releases all retained-node observers.
  @override
  void dispose() {
    _clearDependencies();
    super.dispose();
  }

  /// Observes structural reads while visible rows retain their own subscriptions.
  @override
  Widget build(BuildContext context) {
    _clearDependencies();
    if (node.isDisposed) {
      return const SliverToBoxAdapter(child: SizedBox.shrink());
    }
    final reads = <_ComposeDslNode>{};
    try {
      return _ComposeDslReadScope.track(reads, _sliverContent);
    } finally {
      for (final dependency in reads) {
        if (!dependency.isDisposed && _dependencies.add(dependency)) {
          dependency.addListener(_invalidate);
        }
      }
    }
  }

  /// Preserves box roots and exposes static lazy-column items to the owning viewport.
  Widget _sliverContent() {
    if (node.type != 'LazyColumn') {
      return SliverToBoxAdapter(child: widget.renderer);
    }
    if (node.props['viewport'] != null ||
        node.props['reverseLayout'] == true ||
        node.props['autoScrollToEnd'] == true) {
      throw StateError(
        'Embedded LazyColumn content uses the parent viewport and scroll direction; '
        'independent viewport windows and automatic scrolling are unsupported.',
      );
    }
    final items = <_ComposeDslNode>[];
    final dialogs = <_ComposeDslNode>[];
    final content = node.slots['content'];
    final children = content != null && content.isNotEmpty
        ? content
        : node.children;
    for (final child in children) {
      if (child.type == 'Dialog' || child.type == 'AlertDialog') {
        dialogs.add(child);
      } else {
        items.add(child);
      }
    }
    final indices = <Object, int>{};
    for (var index = 0; index < items.length; index++) {
      final key = items[index].props['key'];
      if (key != null) {
        if (indices.containsKey(key)) {
          throw StateError('Duplicate Compose list key: $key');
        }
        indices[key] = index;
      }
    }
    final spacing = _number(node.props['spacing']) ?? 0;
    final alignment = Alignment(switch (_normalizeToken(
      _string(node.props['horizontalAlignment']),
    )) {
      'center' || 'centerhorizontally' => 0,
      'end' || 'right' => 1,
      _ => -1,
    }, -1);
    EdgeInsetsGeometry padding =
        _commonPaddingFromProps(node.props) ?? EdgeInsets.zero;
    for (final operation in _modifierOps(node.props['modifier'])) {
      if (_normalizeToken(_string(operation['name'])) == 'padding') {
        padding = padding.add(_edgeInsets(operation['args'] as List<Object?>));
      }
    }
    return SliverMainAxisGroup(
      slivers: [
        // Modal nodes own routes and must stay mounted even when the user is
        // scrolling far from the row that opened them.
        for (var index = 0; index < dialogs.length; index++)
          SliverToBoxAdapter(
            key: dialogs[index].props['key'] == null
                ? null
                : ValueKey((
                    'compose-sliver-dialog',
                    dialogs[index].props['key'],
                  )),
            child: _sliverItem(dialogs[index], 'dialog/$index'),
          ),
        SliverPadding(
          key: const ValueKey('compose-sliver-items'),
          padding: padding,
          sliver: SliverList(
            delegate: SliverChildBuilderDelegate(
              (context, index) => KeyedSubtree(
                key: items[index].props['key'] == null
                    ? null
                    : ValueKey(items[index].props['key']),
                child: Padding(
                  padding: EdgeInsets.only(
                    bottom: index + 1 < items.length ? spacing : 0,
                  ),
                  child: Align(
                    alignment: alignment,
                    heightFactor: 1,
                    child: _sliverItem(items[index], 'content/$index'),
                  ),
                ),
              ),
              childCount: items.length,
              findChildIndexCallback: (key) =>
                  key is ValueKey ? indices[key.value] : null,
            ),
          ),
        ),
      ],
    );
  }

  /// Builds one keyed row or persistent modal through the ordinary node renderer.
  Widget _sliverItem(_ComposeDslNode child, String path) => _ComposeDslRenderer(
    key: child.props['key'] == null ? null : ValueKey(child.props['key']),
    node: child,
    onAction: widget.renderer.onAction,
    onTextInput: widget.renderer.onTextInput,
    webViewHostContext: widget.renderer.webViewHostContext,
    splitMarkdownContent: widget.renderer.splitMarkdownContent,
    dispatchOnLoad: widget.renderer.dispatchOnLoad,
    nodePath: '${widget.renderer.nodePath}:$path',
  );
}
