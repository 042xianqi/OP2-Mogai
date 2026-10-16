import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:operit2/core/logging/ClientLogger.dart';
import 'package:operit2/ui/features/packages/screens/ToolPkgComposeDslWebView.dart';
import 'package:operit2/ui/features/packages/screens/ToolPkgUiLauncherScreen.dart';

/// Creates mixed-height keyed items resembling role headings and conversation rows.
Map<String, Object?> _tree({int count = 1000, bool dialog = false}) => {
  'type': 'LazyColumn',
  'props': {'fillMaxWidth': true, 'paddingBottom': 16, 'spacing': 2},
  'children': [
    for (var index = 0; index < count; index++)
      {
        'type': 'Row',
        'props': {
          'key': 'chat-$index',
          'height': index % 10 == 0 ? 46 : 34,
          'fillMaxWidth': true,
        },
        'children': [
          {
            'type': 'Text',
            'props': {'text': 'Chat $index'},
          },
        ],
      },
    if (dialog)
      {
        'type': 'Dialog',
        'props': {'key': 'edit-chat'},
        'children': [
          {
            'type': 'Text',
            'props': {'text': 'Edit conversation'},
          },
        ],
      },
  ],
};

/// Uses the production renderer inside the same single viewport as host chrome.
Widget _surface(
  Map<String, Object?> tree,
  ScrollController controller, {
  String? error,
  List<String>? actions,
}) => MaterialApp(
  home: Scaffold(
    body: Center(
      child: SizedBox(
        width: 320,
        height: 400,
        child: CustomScrollView(
          controller: controller,
          slivers: [
            const SliverToBoxAdapter(child: Text('Sidebar header')),
            buildComposeDslHostForTest(
              node: tree,
              contentBuilder: (context, content) => content.sliver,
              error: error,
              hostContext: ComposeDslWebViewHostContext(
                packageName: 'fixture.sliver',
                routeInstanceId: 'sliver-test',
                executionContextKey: 'sliver-test',
                dispatchAction: (id, [payload]) async {
                  actions?.add(id);
                  return null;
                },
                runtimeOptionsProvider: () => {},
              ),
            ),
            const SliverToBoxAdapter(child: Text('Sidebar footer')),
          ],
        ),
      ),
    ),
  ),
);

/// Counts mounted conversation labels, including the viewport's nearby cache.
int _mountedChats() => find
    .byWidgetPredicate(
      (widget) =>
          widget is Text && RegExp(r'^Chat \d+$').hasMatch(widget.data ?? ''),
    )
    .evaluate()
    .length;

/// Verifies viewport ownership, item virtualization, and independent modal lifetime.
void main() {
  setUpAll(ClientLogger.initialize);

  testWidgets(
    'sliver presentation preserves rows and root loading across action errors',
    (tester) async {
      final controller = ScrollController();
      addTearDown(controller.dispose);
      final actions = <String>[];
      final tree = _tree(count: 20);
      (tree['props'] as Map)['onLoad'] = {'__actionId': 'root-load'};
      await tester.pumpWidget(_surface(tree, controller, actions: actions));
      await tester.pumpAndSettle();
      final row = tester.element(find.text('Chat 0'));
      final position = controller.position;
      expect(actions, ['root-load']);

      await tester.pumpWidget(
        _surface(tree, controller, error: 'Action failed', actions: actions),
      );
      await tester.pumpAndSettle();
      expect(find.text('Action failed', skipOffstage: false), findsOneWidget);
      expect(tester.element(find.text('Chat 0')), same(row));
      expect(controller.position, same(position));

      await tester.pumpWidget(_surface(tree, controller, actions: actions));
      await tester.pumpAndSettle();
      expect(find.text('Action failed', skipOffstage: false), findsNothing);
      expect(tester.element(find.text('Chat 0')), same(row));
      expect(actions, ['root-load']);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('a thousand mixed-height rows mount only the visible cache', (
    tester,
  ) async {
    final controller = ScrollController();
    addTearDown(controller.dispose);
    await tester.pumpWidget(_surface(_tree(), controller));
    await tester.pumpAndSettle();
    expect(find.byType(Scrollable), findsOneWidget);
    expect(find.text('Chat 0'), findsOneWidget);
    expect(find.text('Chat 999'), findsNothing);
    expect(_mountedChats(), inInclusiveRange(1, 40));
    expect(
      tester.state(
        find.ancestor(
          of: find.text('Sidebar header'),
          matching: find.byType(Scrollable),
        ),
      ),
      same(
        tester.state(
          find.ancestor(
            of: find.text('Sidebar footer', skipOffstage: false),
            matching: find.byType(Scrollable),
          ),
        ),
      ),
    );

    await tester.scrollUntilVisible(
      find.text('Chat 999'),
      2500,
      scrollable: find.byType(Scrollable),
      maxScrolls: 30,
    );
    await tester.pumpAndSettle();
    expect(find.text('Chat 999'), findsOneWidget);
    expect(find.text('Chat 0'), findsNothing);
    expect(_mountedChats(), inInclusiveRange(1, 40));

    controller.jumpTo(0);
    await tester.pumpAndSettle();
    expect(find.text('Chat 0'), findsOneWidget);
    expect(find.text('Chat 999'), findsNothing);
    expect(_mountedChats(), inInclusiveRange(1, 40));
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'a trailing modal mounts without scrolling to its list position',
    (tester) async {
      final controller = ScrollController();
      addTearDown(controller.dispose);
      await tester.pumpWidget(_surface(_tree(dialog: true), controller));
      await tester.pumpAndSettle();
      expect(find.text('Edit conversation'), findsOneWidget);
      expect(find.text('Chat 999'), findsNothing);
      expect(_mountedChats(), lessThan(40));

      controller.jumpTo(12000);
      await tester.pumpAndSettle();
      expect(find.text('Edit conversation'), findsOneWidget);
      await tester.pumpWidget(_surface(_tree(), controller));
      await tester.pumpAndSettle();
      expect(find.text('Edit conversation'), findsNothing);
      expect(controller.offset, 12000);
      expect(tester.takeException(), isNull);
    },
  );
}
