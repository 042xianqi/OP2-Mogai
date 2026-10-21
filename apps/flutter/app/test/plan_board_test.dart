// 计划模式单元 + 组件测试。
//
// 覆盖三层：
//   1. PlanStepParser：模型输出不稳定时的防御性解析（含坏 JSON 不得伪造步骤）
//   2. PlanBoardDeriver：从消息流派生活跃计划（增/改/清、别名、去重、非法状态）
//   3. StickyPlanBar：真实渲染（步骤行/进度/折叠展开）

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:operit2/core/proxy/generated/CoreProxyModels.g.dart'
    as core_proxy;
import 'package:operit2/ui/features/chat/components/plan/PlanBoardDeriver.dart';
import 'package:operit2/ui/features/chat/components/plan/PlanStepParser.dart';
import 'package:operit2/ui/features/chat/components/plan/StickyPlanBar.dart';

int _seq = 0;

/// 用 fromJson 构造消息：与运行时真实数据同路径，
/// 也避免逐个补齐 ChatMessage 的 18 个必填字段。
core_proxy.ChatMessage _message({
  String sender = 'ai',
  required List<Map<String, Object?>> parts,
}) {
  return core_proxy.ChatMessage.fromJson(<String, Object?>{
    'sender': sender,
    'parts': parts,
    'timestamp': _seq++,
    'roleName': sender,
    'pluginExtensions': <String, Object?>{},
    'selectedVariantIndex': 0,
    'variantCount': 1,
    'provider': '',
    'modelName': '',
    'inputTokens': 0,
    'outputTokens': 0,
    'cachedInputTokens': 0,
    'sentAt': 0,
    'outputDurationMs': 0,
    'waitDurationMs': 0,
    'completedAt': 0,
    'displayMode': 'NORMAL',
    'isFavorite': false,
    'contentStream': null,
  });
}

Map<String, Object?> _planPart(
  String action, {
  String? goal,
  String? steps,
  String? status,
  String? callId,
}) {
  final Map<String, Object?> attributes = <String, Object?>{'action': action};
  if (goal != null) attributes['goal'] = goal;
  if (steps != null) attributes['steps'] = steps;
  if (status != null) attributes['status'] = status;
  return <String, Object?>{
    'partId': 'part-${_seq++}',
    'sequence': 0,
    'kind': 'tool_call',
    'content': '',
    'toolCallId': callId ?? 'call-${_seq++}',
    'toolName': PlanBoardDeriver.toolName,
    'attributes': attributes,
  };
}

core_proxy.ChatMessage _planCall(
  String action, {
  String? goal,
  String? steps,
  String? status,
  String? callId,
}) {
  return _message(
    parts: <Map<String, Object?>>[
      _planPart(action,
          goal: goal, steps: steps, status: status, callId: callId),
    ],
  );
}

core_proxy.ChatMessage _plain(String text) {
  return _message(
    sender: 'user',
    parts: <Map<String, Object?>>[
      <String, Object?>{
        'partId': 'part-${_seq++}',
        'sequence': 0,
        'kind': 'markdown',
        'content': text,
        'toolCallId': null,
        'toolName': null,
        'attributes': <String, Object?>{},
      },
    ],
  );
}

void main() {
  group('PlanStepParser', () {
    test('规范对象数组', () {
      final List<PlanStepUi> steps = PlanStepParser.parse(
        '[{"id":"1","title":"读代码","status":"completed"},'
        '{"id":"2","title":"改触发","status":"in_progress"}]',
      );
      expect(steps.length, 2);
      expect(steps.first.isCompleted, isTrue);
      expect(steps[1].isCompleted, isFalse);
      expect(PlanStepParser.activeStep(steps)?.title, '改触发');
    });

    test('布尔 done + 别名字段', () {
      final List<PlanStepUi> steps = PlanStepParser.parse(
        '[{"step":"甲","done":true},{"name":"乙","done":false}]',
      );
      expect(steps[0].isCompleted, isTrue);
      expect(steps[0].title, '甲');
      expect(steps[1].title, '乙');
    });

    test('纯字符串数组全部 pending', () {
      final List<PlanStepUi> steps = PlanStepParser.parse('["甲","乙","丙"]');
      expect(steps.length, 3);
      expect(steps.every((PlanStepUi s) => !s.isCompleted), isTrue);
    });

    test('{"steps":[...]} 包裹可解', () {
      final List<PlanStepUi> steps =
          PlanStepParser.parse('{"goal":"x","steps":["甲"]}');
      expect(steps.length, 1);
      expect(steps.first.title, '甲');
    });

    test('坏 JSON 必须降级为空，不得伪造步骤', () {
      expect(PlanStepParser.parse('{oops'), isEmpty);
      expect(PlanStepParser.parse('[1,'), isEmpty);
      expect(PlanStepParser.parse(null), isEmpty);
      expect(PlanStepParser.parse('   '), isEmpty);
    });

    test('markdown 列表行按行解析', () {
      final List<PlanStepUi> steps = PlanStepParser.parse(
        '- [x] 已完成项\n- [ ] 未完成项\n2. 编号项',
      );
      expect(steps.length, 3);
      expect(steps[0].isCompleted, isTrue);
      expect(steps[1].title, '未完成项');
    });

    test('进度计算', () {
      final List<PlanStepUi> steps = PlanStepParser.parse(
        '[{"title":"a","status":"completed"},{"title":"b","status":"pending"},'
        '{"title":"c","status":"done"},{"title":"d","status":"pending"}]',
      );
      expect(PlanStepParser.progress(steps), closeTo(0.5, 1e-9));
    });
  });

  group('PlanBoardDeriver', () {
    test('空历史与无 plan 调用都返回 null', () {
      expect(PlanBoardDeriver.derive(const <core_proxy.ChatMessage>[]), isNull);
      expect(
        PlanBoardDeriver.derive(<core_proxy.ChatMessage>[
          _plain('你好'),
          _plain('在吗'),
        ]),
        isNull,
      );
    });

    test('replace_active 建立计划', () {
      final ActivePlan? plan = PlanBoardDeriver.derive(<core_proxy.ChatMessage>[
        _plain('干活'),
        _planCall(
          'replace_active',
          goal: '修压缩',
          steps: '[{"id":"1","title":"读码","status":"completed"},'
              '{"id":"2","title":"改触发","status":"in_progress"}]',
        ),
      ]);
      expect(plan, isNotNull);
      expect(plan!.goal, '修压缩');
      expect(plan.steps.length, 2);
      expect(plan.status, 'active');
    });

    test('advance 覆盖步骤并回填 goal', () {
      final ActivePlan? plan = PlanBoardDeriver.derive(<core_proxy.ChatMessage>[
        _planCall('replace_active', goal: 'g', steps: '["a","b"]'),
        _planCall('advance', steps: '["a","b"]'),
      ]);
      expect(plan, isNotNull);
      expect(plan!.goal, 'g');
      expect(plan.steps.length, 2);
    });

    test('clear_active 清空', () {
      final ActivePlan? plan = PlanBoardDeriver.derive(<core_proxy.ChatMessage>[
        _planCall('create', goal: 'x', steps: '["a"]'),
        _planCall('clear_active'),
      ]);
      expect(plan, isNull);
    });

    test('顶层 completed 视为结束', () {
      final ActivePlan? plan = PlanBoardDeriver.derive(<core_proxy.ChatMessage>[
        _planCall('create', goal: 'x', steps: '["a"]'),
        _planCall('advance', steps: '["a"]', status: 'completed'),
      ]);
      expect(plan, isNull);
    });

    test('非法 status 回落 active', () {
      final ActivePlan? plan = PlanBoardDeriver.derive(<core_proxy.ChatMessage>[
        _planCall('advance',
            steps: '["a"]', status: 'Rust 37项已过，进入APK构建'),
      ]);
      expect(plan, isNotNull);
      expect(plan!.status, 'active');
    });

    test('别名 create / update_steps / clear', () {
      expect(
        PlanBoardDeriver.derive(<core_proxy.ChatMessage>[
          _planCall('create', goal: '别名', steps: '["a"]'),
        ])?.goal,
        '别名',
      );
      expect(
        PlanBoardDeriver.derive(<core_proxy.ChatMessage>[
          _planCall('create', goal: 'g', steps: '["a"]'),
          _planCall('update_steps', steps: '["a","b"]'),
        ])?.steps.length,
        2,
      );
      expect(
        PlanBoardDeriver.derive(<core_proxy.ChatMessage>[
          _planCall('create', goal: 'g', steps: '["a"]'),
          _planCall('clear'),
        ]),
        isNull,
      );
    });

    test('同 toolCallId 只算一次', () {
      final ActivePlan? plan = PlanBoardDeriver.derive(<core_proxy.ChatMessage>[
        _planCall('create', goal: 'g1', steps: '["a"]', callId: 'SAME'),
        _planCall('create', goal: 'g2', steps: '["b"]', callId: 'SAME'),
      ]);
      expect(plan?.goal, 'g2');
    });

    test('非 plan 工具调用不干扰', () {
      final core_proxy.ChatMessage other = _message(
        parts: <Map<String, Object?>>[
          <String, Object?>{
            'partId': 'part-${_seq++}',
            'sequence': 0,
            'kind': 'tool_call',
            'content': '',
            'toolCallId': 'call-${_seq++}',
            'toolName': 'read_file',
            'attributes': <String, Object?>{'path': '/tmp/a'},
          },
        ],
      );
      final ActivePlan? plan = PlanBoardDeriver.derive(<core_proxy.ChatMessage>[
        _planCall('create', goal: 'g', steps: '["a"]'),
        other,
      ]);
      expect(plan?.goal, 'g');
    });
  });

  group('StickyPlanBar', () {
    Widget wrap(Widget child) {
      return MaterialApp(home: Scaffold(body: child));
    }

    testWidgets('折叠态：显示当前步骤与真实进度格式', (WidgetTester tester) async {
      final List<PlanStepUi> steps = PlanStepParser.parse(
        '[{"title":"读代码","status":"completed"},'
        '{"title":"改触发","status":"in_progress"},'
        '{"title":"跑测试","status":"pending"}]',
      );
      await tester.pumpWidget(wrap(StickyPlanBar(goal: '修好压缩', steps: steps)));
      await tester.pump(const Duration(milliseconds: 300));

      // 折叠态标题 = 当前步骤（首个未完成），而不是 goal。
      expect(find.textContaining('改触发'), findsWidgets);
      // 进度文本真实格式是 "1 / 3"（带空格），不是 "1/3"。
      expect(find.textContaining('1 / 3'), findsWidgets);
      // 折叠态不展示步骤列表。
      expect(find.textContaining('读代码'), findsNothing);
    });

    testWidgets('展开后展示目标与全部步骤', (WidgetTester tester) async {
      final List<PlanStepUi> steps = PlanStepParser.parse(
        '[{"title":"读代码","status":"completed"},'
        '{"title":"改触发","status":"in_progress"}]',
      );
      await tester.pumpWidget(wrap(StickyPlanBar(goal: '修好压缩', steps: steps)));
      await tester.pump(const Duration(milliseconds: 300));

      expect(find.textContaining('读代码'), findsNothing);

      await tester.tap(find.byType(InkWell).first);
      await tester.pump(const Duration(milliseconds: 400));

      // 展开后：goal 与所有步骤标题都应出现。
      expect(find.textContaining('修好压缩'), findsWidgets);
      expect(find.textContaining('读代码'), findsWidgets);
      expect(find.textContaining('改触发'), findsWidgets);
    });

    testWidgets('空步骤不崩且不占位', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(const StickyPlanBar(goal: '', steps: <PlanStepUi>[])),
      );
      await tester.pump(const Duration(milliseconds: 300));
      expect(tester.takeException(), isNull);
      expect(find.byType(InkWell), findsNothing);
    });

    testWidgets('全部完成时标题切换为完成态', (WidgetTester tester) async {
      final List<PlanStepUi> steps = PlanStepParser.parse(
        '[{"title":"甲","status":"completed"},{"title":"乙","status":"completed"}]',
      );
      await tester.pumpWidget(wrap(StickyPlanBar(goal: '目标', steps: steps)));
      await tester.pump(const Duration(milliseconds: 300));

      expect(find.textContaining('所有计划步骤已完成'), findsWidgets);
      expect(find.textContaining('2 / 2'), findsWidgets);
    });
  });
}
