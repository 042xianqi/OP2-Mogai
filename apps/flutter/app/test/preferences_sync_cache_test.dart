import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:operit2/core/bridge/OperitRuntimeBridge.dart';
import 'package:operit2/core/link/CoreLinkCodec.dart';
import 'package:operit2/core/link/CoreLinkProtocol.dart';
import 'package:operit2/core/proxy/generated/CoreProxyClients.g.dart';
import 'package:operit2/data/preferences/UserPreferencesManager.dart';
import 'package:operit2/ui/common/layout/ApplicationZoomPreferences.dart';
import 'package:operit2/ui/main/layout/SidebarDockController.dart';
import 'package:operit2/ui/main/layout/SidebarDockPreferences.dart';
import 'package:operit2/ui/main/navigation/AppNavigationModels.dart';
import 'package:operit2/ui/theme/OperitTheme.dart';

const _userFile = 'user_preferences.preferences.json';
const _zoomFile = 'application_zoom.preferences.json';
const _dockFile = 'sidebar_dock.preferences.json';

/// Verifies committed state changes reach existing application-level caches.
void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('node appearance changes refresh the live theme', () async {
    final bridge = _PreferenceBridge();
    final clients = GeneratedCoreProxyClients(bridge);
    final savedModes = <ThemeMode>[];
    var changes = 0;
    final controller = OperitThemeController(
      clients: clients,
      preferencesManager: UserPreferencesManager(clients: clients),
      onChanged: () => changes++,
      saveStartupThemeMode: (mode) async => savedModes.add(mode),
    );
    addTearDown(controller.dispose);
    await controller.start();
    await _drainEvents();
    final beforeChange = changes;

    bridge.applyNodeAppearance(<String, Object?>{
      'themeMode': 'dark',
      'fontScale': 1.3,
      'inputStyle': 'classic',
    });
    await _drainEvents();
    expect(controller.themeMode, ThemeMode.dark);
    expect(controller.themePreferenceSnapshot.fontScale, 1.3);
    expect(controller.themePreferenceSnapshot.inputStyle, 'classic');
    expect(changes, greaterThan(beforeChange));
    expect(savedModes.last, ThemeMode.dark);
    expect(bridge.preferenceWrites, 0);

    bridge.applyPeerPreferences(_userFile, <String, Object?>{
      'theme_mode': 'light',
      'font_scale': 2.0,
    });
    await _drainEvents();
    expect(controller.themeMode, ThemeMode.dark);
    expect(controller.themePreferenceSnapshot.fontScale, 1.3);

    bridge.applyNodeAppearance(
      UserPreferencesManager.defaultThemePreferenceSnapshot.toJson(),
    );
    await _drainEvents();
    expect(controller.themeMode, ThemeMode.system);
    expect(controller.themePreferenceSnapshot.fontScale, 1);

    controller.dispose();
    await _drainEvents();
    final afterDispose = changes;
    bridge.applyNodeAppearance(<String, Object?>{'themeMode': 'light'});
    await _drainEvents();
    expect(changes, afterDispose);
    expect(bridge.activeWatches, 0);
  });

  test(
    'an older asynchronous update cannot overwrite the latest theme',
    () async {
      final bridge = _PreferenceBridge();
      final clients = GeneratedCoreProxyClients(bridge);
      Completer<void>? nextModeSave;
      final controller = OperitThemeController(
        clients: clients,
        preferencesManager: UserPreferencesManager(clients: clients),
        onChanged: () {},
        saveStartupThemeMode: (_) async {
          final pending = nextModeSave;
          nextModeSave = null;
          await pending?.future;
        },
      );
      addTearDown(controller.dispose);
      await controller.start();
      await _drainEvents();

      final releaseOldSave = Completer<void>();
      nextModeSave = releaseOldSave;
      bridge.applyNodeAppearance(<String, Object?>{'fontScale': 1.2});
      await _drainEvents();
      expect(nextModeSave, isNull);
      bridge.applyNodeAppearance(<String, Object?>{'fontScale': 1.5});
      await _drainEvents();
      expect(controller.themePreferenceSnapshot.fontScale, 1.5);

      releaseOldSave.complete();
      await _drainEvents();
      expect(controller.themePreferenceSnapshot.fontScale, 1.5);
    },
  );

  test('disposing during startup does not attach a late node watch', () async {
    final bridge = _PreferenceBridge();
    final clients = GeneratedCoreProxyClients(bridge);
    final releaseThemeRead = Completer<void>();
    bridge.nextThemeRead = releaseThemeRead.future;
    final controller = OperitThemeController(
      clients: clients,
      preferencesManager: UserPreferencesManager(clients: clients),
      onChanged: () {},
      saveStartupThemeMode: (_) async {},
    );
    final starting = controller.start();
    await _drainEvents();
    controller.dispose();
    releaseThemeRead.complete();
    await starting;
    await _drainEvents();
    expect(bridge.activeWatches, 0);
  });

  test('peer zoom preferences do not change client-local zoom', () async {
    final bridge = _PreferenceBridge();
    final preferences = ApplicationZoomPreferences(
      clients: GeneratedCoreProxyClients(bridge),
    );
    expect(await preferences.load(), 1);
    await preferences.save(1.1);
    bridge.applyPeerPreferences(_zoomFile, <String, Object?>{'zoom': '1.3'});
    bridge.applyPeerPreferences(_zoomFile, <String, Object?>{'zoom': 'broken'});
    await _drainEvents();
    expect(await preferences.load(), 1.1);
    expect(bridge.preferenceWrites, 0);
    expect(bridge.activeWatches, 0);
  });

  test('node appearance never queries a plugin-owned prompt or card', () async {
    final bridge = _PreferenceBridge();
    bridge.applyPeerPreferences(_userFile, <String, Object?>{
      'theme_mode': 'dark',
      'character_theme_retired_theme_mode': 'dark',
      'character_group_theme_retired_font_scale': '2.0',
    });
    bridge.applyNodeAppearance(<String, Object?>{'themeMode': 'light'});
    final clients = GeneratedCoreProxyClients(bridge);
    final controller = OperitThemeController(
      clients: clients,
      preferencesManager: UserPreferencesManager(clients: clients),
      onChanged: () {},
      saveStartupThemeMode: (_) async {},
    );
    addTearDown(controller.dispose);
    await controller.start();
    await _drainEvents();

    expect(controller.themeMode, ThemeMode.light);
    expect(controller.themePreferenceSnapshot.fontScale, 1);
    expect(bridge.calls.map((call) => call.methodName).toSet(), {
      'initializeNodeThemeAppearance',
      'nodeThemeState',
    });
    expect(
      bridge.watchRequests.map((request) => request.propertyName).toSet(),
      {'nodeThemeStateFlow'},
    );
    expect(bridge.preferenceWrites, 0);
    expect(bridge.preferenceRemovals, 0);
  });

  test('malformed node appearance remains an explicit load error', () async {
    final bridge = _PreferenceBridge();
    bridge.applyNodeAppearance(<String, Object?>{'themeMode': 'invalid-mode'});
    final clients = GeneratedCoreProxyClients(bridge);
    final savedModes = <ThemeMode>[];
    final controller = OperitThemeController(
      clients: clients,
      preferencesManager: UserPreferencesManager(clients: clients),
      onChanged: () {},
      saveStartupThemeMode: (mode) async => savedModes.add(mode),
    );
    addTearDown(controller.dispose);

    await controller.start();
    await _drainEvents();
    expect(controller.themeLoadError, isA<ArgumentError>());
    expect(bridge.appearance['themeMode'], 'invalid-mode');
    expect(savedModes, isEmpty);
    expect(bridge.preferenceWrites, 0);
  });

  test(
    'node appearance writes and resets leave retired preferences untouched',
    () async {
      final bridge = _PreferenceBridge();
      final retired = <String, Object?>{
        'custom_user_avatar_uri': 'host:///avatars/user.png',
        'character_theme_retired_theme_mode': 'dark',
      };
      bridge.applyPeerPreferences(_userFile, retired);
      final clients = GeneratedCoreProxyClients(bridge);
      final preferences = UserPreferencesManager(clients: clients);
      final controller = OperitThemeController(
        clients: clients,
        preferencesManager: preferences,
        onChanged: () {},
        saveStartupThemeMode: (_) async {},
      );
      addTearDown(controller.dispose);
      await controller.start();

      await preferences.saveThemeSettings(
        themeMode: ThemeMode.light,
        fontScale: 1.2,
        bubbleUserRoundedCornersEnabled: false,
      );
      expect(bridge.calls.last.methodName, 'updateNodeThemeAppearance');
      expect(bridge.calls.last.args, <String, Object?>{
        'updates': <String, String>{
          'themeMode': '"light"',
          'fontScale': '1.2',
          'bubbleUserRoundedCornersEnabled': 'false',
        },
      });
      await controller.resetThemeSettings();

      final snapshot = await preferences.resolveThemePreferenceSnapshot();
      expect(
        snapshot.toJson(),
        UserPreferencesManager.defaultThemePreferenceSnapshot.toJson(),
      );
      expect(bridge._values[_userFile], retired);
      expect(bridge.preferenceWrites, 2);
      expect(bridge.preferenceRemovals, 0);
    },
  );

  test(
    'long-paste cache receives sync updates without unrelated notifications',
    () async {
      final bridge = _PreferenceBridge();
      final preferences = UserPreferencesManager(
        clients: GeneratedCoreProxyClients(bridge),
      );
      final values = <LongPastedTextInputSettings>[];
      final errors = <Object>[];
      final subscription = preferences.longPastedTextInputSettingsFlow().listen(
        values.add,
        onError: errors.add,
      );
      await _drainEvents();
      bridge.applyPeerPreferences(_userFile, <String, Object?>{
        'long_pasted_text_input_enabled': false,
        'long_pasted_text_input_threshold': 5000,
      });
      bridge.applyPeerPreferences(_userFile, <String, Object?>{
        'long_pasted_text_input_enabled': false,
        'long_pasted_text_input_threshold': 5000,
        'theme_mode': 'dark',
      });
      bridge.applyPeerPreferences(_userFile, <String, Object?>{
        'long_pasted_text_input_threshold': 'broken',
      });
      await _drainEvents();
      expect(values.length, 2);
      expect(values.last.enabled, isFalse);
      expect(values.last.threshold, 5000);
      expect(errors.single, isA<TypeError>());
      await subscription.cancel();
      expect(bridge.activeWatches, 0);
    },
  );

  test(
    'sidebar cache applies synced layout and deletion without writing back',
    () async {
      final bridge = _PreferenceBridge();
      final controller = SidebarDockController(
        preferences: SidebarDockPreferences(
          clients: GeneratedCoreProxyClients(bridge),
        ),
      );
      controller.synchronize(
        pluginEntries: const <NavigationEntrySpec>[
          NavigationEntrySpec(
            entryId: 'plugin.entry',
            routeId: 'plugin.route',
            surface: NavigationSurface.mainSidebarPlugins,
            title: 'Plugin',
            icon: Icons.extension_outlined,
            kind: NavigationEntryKind.plugin,
          ),
        ],
        routesById: const <String, RouteSpec>{
          'plugin.route': RouteSpec(
            routeId: 'plugin.route',
            runtime: RouteRuntime.toolPkgComposeDsl,
            ownerPackageName: 'plugin',
            toolPkgUiModuleId: 'module',
          ),
        },
      );
      await controller.loadPreferences();
      bridge.applyPeerPreferences(_dockFile, <String, Object?>{
        'layout': const SidebarDockLayout(
          primaryEntryIds: <String>[],
          secondaryEntryIds: <String>['plugin.entry'],
          selectedSecondaryViewId: 'plugin.entry',
        ).toJson(),
      });
      await _drainEvents();
      expect(controller.primaryEntries, isEmpty);
      expect(controller.secondaryViews.single.entry.entryId, 'plugin.entry');
      expect(controller.selectedSecondaryViewId, 'plugin.entry');

      bridge.applyPeerPreferences(_dockFile, <String, Object?>{});
      await _drainEvents();
      expect(controller.primaryEntries.single.entryId, 'plugin.entry');
      expect(controller.secondaryViews, isEmpty);
      expect(
        controller.selectedSecondaryViewId,
        SidebarDockController.workspaceViewId,
      );
      expect(bridge.preferenceWrites, 0);
      controller.dispose();
      await _drainEvents();
      expect(bridge.activeWatches, 0);
    },
  );

  test('initial watch errors retain their original cause', () async {
    final bridge = _PreferenceBridge()
      ..watchError = StateError('peer watch failed');
    final controller = SidebarDockController(
      preferences: SidebarDockPreferences(
        clients: GeneratedCoreProxyClients(bridge),
      ),
    );
    addTearDown(controller.dispose);
    await expectLater(
      controller.loadPreferences(),
      throwsA(same(bridge.watchError)),
    );
  });
}

/// Drains queued watch delivery and asynchronous cache reloads deterministically.
Future<void> _drainEvents() async {
  for (var turn = 0; turn < 8; turn++) {
    await Future<void>.delayed(Duration.zero);
  }
}

/// Supplies exact Core calls and real encoded watch events for preference consumers.
class _PreferenceBridge extends OperitRuntimeBridge {
  final Map<String, Map<String, Object?>> _values =
      <String, Map<String, Object?>>{
        _userFile: <String, Object?>{},
        _zoomFile: <String, Object?>{},
        _dockFile: <String, Object?>{},
      };
  final List<CoreCallRequest> calls = <CoreCallRequest>[];
  final List<CoreWatchRequest> watchRequests = <CoreWatchRequest>[];
  final Map<CoreWatchRequest, StreamController<CoreEvent>> _watches = {};
  int preferenceWrites = 0;
  String? localZoom;
  int preferenceRemovals = 0;
  Future<void>? nextThemeRead;
  Object? watchError;

  final Map<String, Object?> appearance = UserPreferencesManager
      .defaultThemePreferenceSnapshot
      .toJson();

  /// Encodes the node-local appearance observed by generated theme clients.
  Map<String, Object?> get nodeState => <String, Object?>{
    'schemaVersion': 1,
    'selection': null,
    'privateAppearanceJson': jsonEncode(appearance),
  };

  /// Publishes a committed partial appearance edit to this node's windows.
  void applyNodeAppearance(Map<String, Object?> updates) {
    appearance.addAll(updates);
    for (final entry in _watches.entries) {
      if (entry.key.propertyName == 'nodeThemeStateFlow') {
        entry.value.add(_event(entry.key, 'Changed', nodeState));
      }
    }
  }

  /// Reports the number of active physical watches after cancellation.
  int get activeWatches => _watches.length;

  /// Publishes a committed peer snapshot without invoking a local preference setter.
  void applyPeerPreferences(String fileName, Map<String, Object?> values) {
    _values[fileName] = Map<String, Object?>.of(values);
    for (final entry in _watches.entries) {
      if (entry.key.propertyName == 'preferencesFlow' &&
          (entry.key.args as Map)['fileName'] == fileName) {
        entry.value.add(_event(entry.key, 'Changed', values));
      }
    }
  }

  /// Implements only the exact reads and writes required by the cache owners.
  @override
  Future<Uint8List> callBytes(CoreCallRequest request) async {
    calls.add(request);
    final args = request.args as Map;
    switch (request.methodName) {
      case 'applicationZoomPath':
        expect(request.target, 'core/repository.runtimeStorageRepository');
        return encodeCoreLink(<Object?>[
          0,
          'runtime/client/application_zoom.local',
        ]);
      case 'readText':
        expect(request.target, 'core/repository.runtimeStorageRepository');
        expect(args['path'], 'runtime/client/application_zoom.local');
        return encodeCoreLink(<Object?>[0, localZoom]);
      case 'writeText':
        expect(request.target, 'core/repository.runtimeStorageRepository');
        expect(args['path'], 'runtime/client/application_zoom.local');
        localZoom = args['content'] as String;
        return encodeCoreLink(<Object?>[0, null]);
      case 'getPreferences':
        final values = _values[args['fileName']]!;
        final selected = <String, Object?>{
          for (final key in (args['keys'] as List).cast<String>())
            if (values.containsKey(key)) key: values[key],
        };
        return encodeCoreLink(<Object?>[0, selected]);
      case 'initializeNodeThemeAppearance':
        expect(request.target, 'core/application');
        return encodeCoreLink(<Object?>[0, nodeState]);
      case 'nodeThemeState':
        expect(request.target, 'core/application');
        final state = nodeState;
        final wait = nextThemeRead;
        nextThemeRead = null;
        await wait;
        return encodeCoreLink(<Object?>[0, state]);
      case 'updateNodeThemeAppearance':
        expect(request.target, 'core/application');
        preferenceWrites++;
        final updates = Map<String, String>.from(args['updates'] as Map);
        applyNodeAppearance(<String, Object?>{
          for (final entry in updates.entries)
            entry.key: jsonDecode(entry.value),
        });
        return encodeCoreLink(<Object?>[0, nodeState]);
      case 'removePreferences':
        preferenceRemovals++;
        for (final key in (args['keys'] as List).cast<String>()) {
          _values[args['fileName']]!.remove(key);
        }
        return encodeCoreLink(<Object?>[0, null]);
      case 'setPreferences':
        preferenceWrites++;
        _values[args['fileName']]!.addAll(
          (args['values'] as Map).cast<String, Object?>(),
        );
        return encodeCoreLink(<Object?>[0, null]);
      default:
        throw StateError(
          'Unexpected Core call: ${request.target}.${request.methodName}',
        );
    }
  }

  /// Opens a typed node or preference watch with its initial committed snapshot.
  @override
  Stream<CoreEvent> watchStream(CoreWatchRequest request) {
    watchRequests.add(request);
    if (watchError case final Object error) {
      return Stream<CoreEvent>.error(error);
    }
    final value = switch (request.propertyName) {
      'nodeThemeStateFlow' => nodeState,
      'preferencesFlow' => Map<String, Object?>.of(
        _values[(request.args as Map)['fileName']]!,
      ),

      _ => throw StateError('Unexpected Core watch: ${request.propertyName}'),
    };
    late final StreamController<CoreEvent> controller;
    controller = StreamController<CoreEvent>(
      onListen: () => controller.add(_event(request, 'Snapshot', value)),
      onCancel: () => _watches.remove(request),
    );
    _watches[request] = controller;
    return controller.stream;
  }

  /// Rejects push operations outside the preference cache contract.
  @override
  Future<CorePushSink> push(CorePushRequest request) =>
      throw UnimplementedError();

  /// Rejects snapshot calls outside the preference cache contract.
  @override
  Future<CoreEvent> watchSnapshot(CoreWatchRequest request) =>
      throw UnimplementedError();
}

/// Encodes a real Link watch payload consumed by generated typed clients.
CoreEvent _event(CoreWatchRequest request, String kind, Object? value) {
  return CoreEvent.raw(
    requestId: request.requestId,
    target: request.target,
    propertyName: request.propertyName,
    kind: kind,
    valueBytes: encodeCoreLink(value),
    decodeValue: (bytes) => decodeCoreLink<Object?>(bytes),
  );
}
