// ignore_for_file: file_names
// apps/flutter/app/lib/ui/features/chat/components/plan/StickyPlanBar.dart
//
// 移植自太墟 StickyPlanBar（Kotlin/Compose）
//   /workspace/taixu-ref/feature/chat/src/main/java/top/wkbin/taixu/ui/chat/PlanBoard.kt:256-...
//
// 挂载位置：聊天消息列表与输入框「之间」，即「常驻置底、随消息滚动始终可见」。
// 视觉契约（对齐太墟，不抄配色 → 全部走 Operit2 自己的 ColorScheme）：
//   - 默认折叠；点标题行展开/收起，带触感
//   - 折叠态标题三选一：全完成 →「所有计划步骤已完成」；有当前步骤 → 该步骤标题；否则 → goal
//   - 进度 chip「已完成 / 总数」+ 2dp 细进度条常显
//   - 展开区 maxHeight 220 + 垂直滚动（长计划不撑爆屏幕）
//   - 根节点 animateContentSize()，展开/收起用 AnimatedVisibility 等价物
//   - 已完成步骤：实心勾 + 删除线 + 弱化色；未完成：空心描边圆；当前步骤加粗

import 'package:flutter/material.dart';

import 'PlanStepParser.dart';

/// 计划步骤条：贴底显示当前任务的执行进度。
class StickyPlanBar extends StatefulWidget {
  const StickyPlanBar({
    super.key,
    required this.goal,
    required this.steps,
    this.onTapExpand,
  });

  /// 计划总目标（advance 场景下可能为空，则用步骤标题兜底）。
  final String goal;

  /// 已解析的步骤列表。
  final List<PlanStepUi> steps;

  /// 可选：展开状态变化回调（用于埋点/测试）。
  final ValueChanged<bool>? onTapExpand;

  @override
  State<StickyPlanBar> createState() => _StickyPlanBarState();
}

class _StickyPlanBarState extends State<StickyPlanBar> {
  bool _expanded = false;

  void _toggleExpanded() {
    // 与太墟同款触感（TextHandleMove 在移动端是轻震）
    Feedback.forTap(context);
    setState(() => _expanded = !_expanded);
    widget.onTapExpand?.call(_expanded);
  }

  @override
  Widget build(BuildContext context) {
    final List<PlanStepUi> steps = widget.steps;
    if (steps.isEmpty) return const SizedBox.shrink();

    final ThemeData theme = Theme.of(context);
    final ColorScheme colors = theme.colorScheme;

    final int completedCount = steps
        .where((PlanStepUi step) => step.isCompleted)
        .length;
    final bool allDone = completedCount == steps.length;
    final PlanStepUi? activeStep = PlanStepParser.activeStep(steps);
    final double progress = PlanStepParser.progress(steps);

    final Color accent = allDone
        ? colors.outline.withValues(alpha: 0.25)
        : colors.primary.withValues(alpha: 0.35);

    final String headline = allDone
        ? '所有计划步骤已完成'
        : (activeStep?.title ?? (widget.goal.isEmpty ? '任务执行规划' : widget.goal));

    return AnimatedSize(
      duration: const Duration(milliseconds: 180),
      curve: Curves.easeOutCubic,
      alignment: Alignment.bottomCenter,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(12, 2, 12, 2),
        child: Material(
          color: colors.surfaceContainerHighest.withValues(alpha: 0.55),
          borderRadius: const BorderRadius.vertical(top: Radius.circular(10)),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            onTap: _toggleExpanded,
            child: Container(
              decoration: BoxDecoration(
                border: Border.all(color: accent, width: 1),
                borderRadius: const BorderRadius.vertical(
                  top: Radius.circular(10),
                ),
              ),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  // ── 折叠态标题行 ───────────────────────────────
                  Padding(
                    padding: const EdgeInsets.fromLTRB(10, 6, 8, 6),
                    child: Row(
                      children: <Widget>[
                        Icon(
                          allDone ? Icons.check_circle_outline : Icons.list_alt,
                          size: 15,
                          color: allDone
                              ? colors.onSurfaceVariant
                              : colors.primary,
                        ),
                        const SizedBox(width: 6),
                        Expanded(
                          child: Text(
                            headline,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: theme.textTheme.labelMedium?.copyWith(
                              fontWeight: FontWeight.w600,
                              color: allDone
                                  ? colors.onSurfaceVariant
                                  : colors.onSurface,
                            ),
                          ),
                        ),
                        const SizedBox(width: 6),
                        Text(
                          '$completedCount / ${steps.length}',
                          style: theme.textTheme.labelSmall?.copyWith(
                            color: colors.onSurfaceVariant,
                            fontFeatures: const <FontFeature>[
                              FontFeature.tabularFigures(),
                            ],
                          ),
                        ),
                        Icon(
                          _expanded ? Icons.expand_more : Icons.expand_less,
                          size: 18,
                          color: colors.onSurfaceVariant,
                        ),
                      ],
                    ),
                  ),

                  // ── 细进度条（常显） ───────────────────────────
                  SizedBox(
                    height: 2,
                    child: LinearProgressIndicator(
                      value: progress,
                      minHeight: 2,
                      backgroundColor: colors.outlineVariant.withValues(
                        alpha: 0.35,
                      ),
                      valueColor: AlwaysStoppedAnimation<Color>(
                        allDone ? colors.outline : colors.primary,
                      ),
                    ),
                  ),

                  // ── 展开区 ───────────────────────────────────
                  if (_expanded)
                    ConstrainedBox(
                      constraints: const BoxConstraints(maxHeight: 220),
                      child: SingleChildScrollView(
                        padding: const EdgeInsets.fromLTRB(10, 6, 10, 8),
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: <Widget>[
                            if (widget.goal.isNotEmpty && !allDone) ...<Widget>[
                              Text(
                                widget.goal,
                                style: theme.textTheme.labelSmall?.copyWith(
                                  color: colors.onSurfaceVariant,
                                ),
                              ),
                              const SizedBox(height: 6),
                            ],
                            for (final PlanStepUi step in steps)
                              _StepRow(
                                step: step,
                                isActive: !allDone && step == activeStep,
                              ),
                          ],
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// 单步渲染：完成=实心勾+删除线；未完成=空心圆；当前步骤加粗。
class _StepRow extends StatelessWidget {
  const _StepRow({required this.step, required this.isActive});

  final PlanStepUi step;
  final bool isActive;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final ColorScheme colors = theme.colorScheme;

    final Widget marker = step.isCompleted
        ? Icon(Icons.check_circle, size: 13, color: colors.primary)
        : Container(
            width: 12,
            height: 12,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              border: Border.all(
                color: isActive ? colors.primary : colors.outline,
                width: 1.5,
              ),
            ),
          );

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Padding(padding: const EdgeInsets.only(top: 2), child: marker),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              step.title,
              style: theme.textTheme.bodySmall?.copyWith(
                color: step.isCompleted
                    ? colors.onSurfaceVariant
                    : colors.onSurface,
                fontWeight: isActive ? FontWeight.w600 : FontWeight.w400,
                decoration: step.isCompleted
                    ? TextDecoration.lineThrough
                    : TextDecoration.none,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
