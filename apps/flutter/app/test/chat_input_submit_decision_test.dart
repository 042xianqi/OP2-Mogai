import 'package:flutter_test/flutter_test.dart';
import 'package:operit2/ui/features/chat/viewmodel/ChatViewModel.dart';

/// Verifies the Core chat input bridge decision decodes to canonical actions.
void main() {
  /// Builds the raw JSON decision exactly as Core serializes it.
  Map<String, Object?> wire(String action, {bool clearInput = false}) {
    return <String, Object?>{
      'action': action,
      'text': null,
      'message': null,
      'clearInput': clearInput,
      'timedOut': false,
      'metadata': <String, Object?>{},
    };
  }

  test('keeps the canonical capitalized consume decision intact', () {
    final decision = ChatInputSubmitDecision.fromJson(
      wire('Consume', clearInput: true),
    );
    expect(decision.action, chatInputActionConsume);
    expect(decision.clearInput, isTrue);
  });

  test('normalizes every canonical action case-insensitively', () {
    expect(
      ChatInputSubmitDecision.fromJson(wire('Allow')).action,
      chatInputActionAllow,
    );
    expect(
      ChatInputSubmitDecision.fromJson(wire('Block')).action,
      chatInputActionBlock,
    );
    expect(
      ChatInputSubmitDecision.fromJson(wire('Consume')).action,
      chatInputActionConsume,
    );
    expect(
      ChatInputSubmitDecision.fromJson(wire('Replace')).action,
      chatInputActionReplace,
    );
    expect(
      ChatInputSubmitDecision.fromJson(wire('consume')).action,
      chatInputActionConsume,
    );
  });

  test('preserves unknown action spellings for explicit fall-through', () {
    final decision = ChatInputSubmitDecision.fromJson(wire('Mystery'));
    expect(decision.action, 'Mystery');
  });
}
