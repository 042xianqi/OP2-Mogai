// ignore_for_file: file_names
//
// PlanBoardHost —— 订阅会话消息流并把活跃计划渲染成输入框上方的计划条。
//
// 数据来源：ChatViewModel.watchMessages(chatId) 返回的消息流。
// 计划状态从这些消息里的 plan 工具调用派生（见 PlanBoardDeriver），
// 因此无需新建数据库表：会话切走再切回、App 重启后计划条都能复原。

import 'dart:async';

import 'package:flutter/widgets.dart';

import '../../../../../core/proxy/generated/CoreProxyModels.g.dart'
    as core_proxy;
import '../../viewmodel/ChatViewModel.dart';
import 'PlanBoardDeriver.dart';
import 'PlanStepParser.dart';
import 'StickyPlanBar.dart';

/// 计划条容器：只负责取数，渲染交给 [StickyPlanBar]。
class PlanBoardHost extends StatefulWidget {
  const PlanBoardHost({
    super.key,
    required this.chatId,
    required this.viewModel,
  });

  final String? chatId;
  final ChatViewModel viewModel;

  @override
  State<PlanBoardHost> createState() => _PlanBoardHostState();
}

class _PlanBoardHostState extends State<PlanBoardHost> {
  StreamSubscription<List<core_proxy.ChatMessage>>? _subscription;
  ActivePlan? _plan;

  @override
  void initState() {
    super.initState();
    _resubscribe();
  }

  @override
  void didUpdateWidget(covariant PlanBoardHost oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.chatId != widget.chatId ||
        oldWidget.viewModel != widget.viewModel) {
      _resubscribe();
    }
  }

  @override
  void dispose() {
    _subscription?.cancel();
    super.dispose();
  }

  void _resubscribe() {
    _subscription?.cancel();
    _subscription = null;
    _plan = null;

    final String? chatId = widget.chatId;
    if (chatId == null || chatId.isEmpty) {
      return;
    }
    _subscription = widget.viewModel.watchMessages(chatId).listen(
      (List<core_proxy.ChatMessage> messages) {
        if (!mounted) {
          return;
        }
        final ActivePlan? next = PlanBoardDeriver.derive(messages);
        // 计划未变化时不 setState，避免每条消息都重建计划条。
        if (_samePlan(_plan, next)) {
          return;
        }
        setState(() => _plan = next);
      },
      onError: (Object error, StackTrace stackTrace) {
        // 消息流异常不应该把聊天页打崩，静默隐藏计划条即可。
        if (!mounted) {
          return;
        }
        setState(() => _plan = null);
      },
    );
  }

  static bool _samePlan(ActivePlan? left, ActivePlan? right) {
    if (identical(left, right)) {
      return true;
    }
    if (left == null || right == null) {
      return false;
    }
    if (left.status != right.status ||
        left.goal != right.goal ||
        left.steps.length != right.steps.length) {
      return false;
    }
    for (int index = 0; index < left.steps.length; index++) {
      final PlanStepUi a = left.steps[index];
      final PlanStepUi b = right.steps[index];
      if (a.index != b.index ||
          a.title != b.title ||
          a.isCompleted != b.isCompleted ||
          a.status != b.status) {
        return false;
      }
    }
    return true;
  }

  @override
  Widget build(BuildContext context) {
    final ActivePlan? plan = _plan;
    if (plan == null) {
      return const SizedBox.shrink();
    }
    return StickyPlanBar(
      goal: plan.goal,
      steps: plan.steps,
    );
  }
}
