// ignore_for_file: file_names
// apps/flutter/app/lib/ui/features/chat/components/plan/PlanStepParser.dart
//
// 移植自太墟 PlanStepParser（Kotlin，带 6 个单测锁定行为）
//   /workspace/taixu-ref/feature/chat/src/main/java/top/wkbin/taixu/ui/chat/PlanBoard.kt:60-142
//
// 职责：把 plan 工具的 steps 参数（模型产出的、形状不受控的 JSON）解析成稳定的步骤列表。
// 铁律：任何输入都不许抛异常 —— 解析失败一律降级为空列表，看板绝不崩。

import 'dart:convert';

/// 单个步骤的 UI 模型。
///
/// 注意：太墟在这里把 4 态（pending/in_progress/completed/failed）塌缩成布尔 [isCompleted]，
/// 「当前步骤」= 列表里第一个未完成项，依赖模型保证数组顺序。
/// 本移植保留这个契约，另外额外保留 [status] 原文，方便将来做 4 态视觉而不改解析器。
class PlanStepUi {
  const PlanStepUi({
    required this.index,
    required this.title,
    required this.isCompleted,
    this.status = '',
  });

  /// 从 1 开始的序号，用于步骤行左侧标号。
  final int index;

  /// 步骤标题（已 trim，非空）。
  final String title;

  /// 是否已完成。
  final bool isCompleted;

  /// 原始状态字符串（pending / in_progress / completed / failed / 空）。
  final String status;
}

/// plan 工具写入的 steps 防御性解析器。
class PlanStepParser {
  PlanStepParser._();

  /// 标题字段别名，与太墟 TITLE_KEYS 完全一致。
  static const List<String> _titleKeys = <String>[
    'title',
    'step',
    'name',
    'description',
    'desc',
    'task',
    'content',
    'action',
  ];

  /// 状态字段别名。
  static const List<String> _statusKeys = <String>[
    'status',
    'state',
    'completed',
    'isCompleted',
    'is_completed',
    'done',
    'finished',
  ];

  /// 视为「已完成」的状态字符串。
  static const Set<String> _completedStatuses = <String>{
    'completed',
    'complete',
    'done',
    'finished',
  };

  /// 解析 steps 原始文本（可为 JSON 字符串、也可能是已被上层解析好的 JSON 文本）。
  ///
  /// 支持形状：
  /// - 裸数组 `[{...}]` 或 `["甲","乙"]`
  /// - 对象包裹 `{"steps":[...]}`
  /// - 非法 JSON / 空 → 空列表
  static List<PlanStepUi> parse(String? stepsJson) {
    if (stepsJson == null) return const <PlanStepUi>[];
    final String trimmed = stepsJson.trim();
    if (trimmed.isEmpty) return const <PlanStepUi>[];

    dynamic root;
    try {
      root = jsonDecode(trimmed);
    } catch (_) {
      // 不是合法 JSON：
      // - 若它「看起来本该是 JSON」（以 { 或 [ 开头）→ 判定为坏数据，降级空列表，
      //   绝不能把 "{oops" 这种残片当成一条步骤（太墟同款铁律：非法 JSON → 空）。
      // - 否则视为模型直接吐的 markdown/纯文本列表，按行解析反而更有用。
      if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
        return const <PlanStepUi>[];
      }
      return _parseLines(trimmed);
    }
    return _parseElement(root);
  }

  static List<PlanStepUi> _parseElement(dynamic element) {
    if (element is List) {
      return _parseList(element);
    }
    if (element is Map) {
      final dynamic nested = element['steps'];
      if (nested != null) return _parseElement(nested);
    }
    return const <PlanStepUi>[];
  }

  static List<PlanStepUi> _parseList(List<dynamic> items) {
    final List<PlanStepUi> result = <PlanStepUi>[];
    for (final dynamic item in items) {
      if (item is String) {
        final String title = item.trim();
        if (title.isEmpty) continue;
        result.add(
          PlanStepUi(
            index: result.length + 1,
            title: title,
            isCompleted: false,
            status: 'pending',
          ),
        );
        continue;
      }
      if (item is! Map) continue;

      final String title = _resolveTitle(item);
      if (title.isEmpty) continue;

      final String status = _resolveStatus(item);
      result.add(
        PlanStepUi(
          index: result.length + 1,
          title: title,
          isCompleted: _completedStatuses.contains(status),
          status: status,
        ),
      );
    }
    return result;
  }

  /// 按别名取标题；全部落空时取「第一个非状态类字符串值」兜底。
  static String _resolveTitle(Map<dynamic, dynamic> item) {
    for (final String key in _titleKeys) {
      final dynamic value = item[key];
      if (value is String && value.trim().isNotEmpty) return value.trim();
    }
    for (final MapEntry<dynamic, dynamic> entry in item.entries) {
      final String key = entry.key.toString();
      if (_statusKeys.contains(key)) continue;
      final dynamic value = entry.value;
      if (value is String && value.trim().isNotEmpty) return value.trim();
    }
    return '';
  }

  /// 状态兼容布尔（done:true）与枚举字符串。
  static String _resolveStatus(Map<dynamic, dynamic> item) {
    for (final String key in _statusKeys) {
      final dynamic value = item[key];
      if (value is bool) return value ? 'completed' : 'pending';
      if (value is String) {
        final String normalized = value.trim().toLowerCase();
        if (normalized.isNotEmpty) return normalized;
      }
    }
    return '';
  }

  static List<PlanStepUi> _parseLines(String text) {
    final List<PlanStepUi> result = <PlanStepUi>[];
    for (final String rawLine in const LineSplitter().convert(text)) {
      String line = rawLine.trim();
      if (line.isEmpty) continue;
      // 去掉 markdown 列表符号与序号：- [ ] / - [x] / 1. / 2) / * / •
      line = line.replaceFirst(RegExp(r'^[-*•]\s*'), '');
      bool completed = false;
      final RegExpMatch? checkbox = RegExp(r'^\[( |x|X)\]\s*').firstMatch(line);
      if (checkbox != null) {
        completed = checkbox.group(1)!.toLowerCase() == 'x';
        line = line.substring(checkbox.end).trim();
      }
      line = line.replaceFirst(RegExp(r'^\d+[.)]\s*'), '').trim();
      if (line.isEmpty) continue;
      result.add(
        PlanStepUi(
          index: result.length + 1,
          title: line,
          isCompleted: completed,
          status: completed ? 'completed' : 'pending',
        ),
      );
    }
    return result;
  }

  /// 进度比例（0.0–1.0），供进度条使用。
  static double progress(List<PlanStepUi> steps) {
    if (steps.isEmpty) return 0;
    final int done = steps.where((PlanStepUi step) => step.isCompleted).length;
    return done / steps.length;
  }

  /// 当前步骤 = 第一个未完成项（太墟 StickyPlanBar.kt:267 同款语义）。
  static PlanStepUi? activeStep(List<PlanStepUi> steps) {
    for (final PlanStepUi step in steps) {
      if (!step.isCompleted) return step;
    }
    return null;
  }
}
