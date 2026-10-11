import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:operit2/core/bridge/OperitRuntimeBridge.dart';
import 'package:operit2/core/link/CoreLinkCodec.dart';
import 'package:operit2/core/link/CoreLinkProtocol.dart';
import 'package:operit2/core/proxy/generated/CoreProxyClients.g.dart';
import 'package:operit2/ui/features/packages/screens/ComposeDslImageSource.dart';
import 'package:operit2/ui/features/packages/screens/ToolPkgComposeDslWebView.dart';
import 'package:operit2/ui/features/packages/screens/ToolPkgUiLauncherScreen.dart';

final png = base64Decode(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=',
);

void main() {
  test(
    'VFS uses the generated Core binary file service without rewriting the path',
    () async {
      const path = '/app/data/cache/toolpkg_resource_exports/operit-avatar.png';
      final bridge = _VfsFileBridge({
        '__type': 'BinaryFileContentData',
        'path': path,
        'contentBase64': base64Encode(png),
        'size': png.length,
      });
      final bytes = await readComposeDslVfsBytes(
        GeneratedCoreProxyClients(bridge),
        path,
      );
      expect(bytes, orderedEquals(png));
      final request = bridge.calls.single;
      expect(request.methodName, 'executeTool');
      expect((request.args as Map)['tool'], {
        'name': 'read_file_binary',
        'parameters': [
          {'name': 'path', 'value': path},
        ],
      });
    },
  );
  test(
    'Core VFS rejection preserves the original error and rejects nonbinary results',
    () async {
      final result = {'__type': 'StringResultData', 'value': 'not binary'};
      final rejected = _VfsFileBridge(
        result,
        success: false,
        error: 'VFS access denied',
      );
      await expectLater(
        readComposeDslVfsBytes(
          GeneratedCoreProxyClients(rejected),
          '/app/denied.png',
        ),
        throwsA(
          isA<StateError>().having(
            (e) => e.message,
            'message',
            contains('VFS access denied'),
          ),
        ),
      );
      final wrongType = _VfsFileBridge(result);
      await expectLater(
        readComposeDslVfsBytes(
          GeneratedCoreProxyClients(wrongType),
          '/app/wrong.png',
        ),
        throwsA(
          isA<StateError>().having(
            (e) => e.message,
            'message',
            contains('binary file content'),
          ),
        ),
      );
    },
  );
  test(
    'known virtual roots and explicit virtual URIs resolve before OS files',
    () async {
      final cases = <String, String>{
        '/app/data/cache/toolpkg_resource_exports/operit-avatar.png':
            '/app/data/cache/toolpkg_resource_exports/operit-avatar.png',
        '/mnt/macos/Users/test/avatar.png': '/mnt/macos/Users/test/avatar.png',
        'file:///app/data/avatar.png': '/app/data/avatar.png',
        'vfs:///app/data/avatar.png': '/app/data/avatar.png',
        'operit-vfs://app/data/avatar.png': '/app/data/avatar.png',
        'runtime/data/user_assets/avatar.png':
            'runtime/data/user_assets/avatar.png',
      };
      for (final entry in cases.entries) {
        final reads = <String>[];
        final provider = await resolveComposeDslImage(
          entry.key,
          readVfsBytes: (path) async {
            reads.add(path);
            return png;
          },
        );
        expect(reads, [entry.value]);
        expect(provider, isA<MemoryImage>());
        expect((provider as MemoryImage).bytes, same(png));
      }
    },
  );
  test(
    'VFS read failure propagates without fallback to native filesystem',
    () async {
      await expectLater(
        resolveComposeDslImage(
          '/app/data/missing.png',
          readVfsBytes: (_) async => throw StateError('original VFS failure'),
        ),
        throwsStateError,
      );
    },
  );
  test('ordinary data network native file sources do not call VFS', () async {
    Future<Uint8List> noVfs(String path) async =>
        throw StateError('Unexpected VFS: $path');
    expect(
      await resolveComposeDslImage(
        'data:image/png;base64,${base64Encode(png)}',
        readVfsBytes: noVfs,
      ),
      isA<MemoryImage>(),
    );
    expect(
      await resolveComposeDslImage(
        'https://example.test/avatar.png',
        readVfsBytes: noVfs,
      ),
      isA<NetworkImage>(),
    );
    expect(
      await resolveComposeDslImage(
        '/Users/test/avatar.png',
        readVfsBytes: noVfs,
      ),
      isA<FileImage>(),
    );
    expect(
      await resolveComposeDslImage(
        'file:///Users/test/avatar.png',
        readVfsBytes: noVfs,
      ),
      isA<FileImage>(),
    );
  });
  testWidgets(
    'actual DSL VFS image survives repaint without another read or image provider',
    (tester) async {
      final reads = <String>[];
      final host = ComposeDslWebViewHostContext(
        packageName: 'test.images',
        routeInstanceId: 'route',
        executionContextKey: 'session',
        dispatchAction: (_, [payload]) async => null,
        runtimeOptionsProvider: () => {},
        readFileBytes: (path) async {
          reads.add(path);
          return png;
        },
      );
      Widget render(String path) => MaterialApp(
        home: buildComposeDslLayoutForTest(
          node: {
            'type': 'Image',
            'props': {'key': 'avatar', 'path': path, 'width': 44, 'height': 44},
            'children': [],
            'slots': {},
          },
          hostContext: host,
          splitMarkdownContent: (_) async => [],
          onTextInput: (_, text) async => null,
        ),
      );
      await tester.pumpWidget(render('/app/data/cache/avatar.png'));
      await tester.pumpAndSettle();
      expect(reads, ['/app/data/cache/avatar.png']);
      expect(tester.takeException(), isNull);
      final first = tester.widget<Image>(find.byType(Image)).image;
      expect(first, isA<MemoryImage>());
      await tester.pumpWidget(render('/app/data/cache/avatar.png'));
      await tester.pumpAndSettle();
      expect(reads, hasLength(1));
      expect(tester.widget<Image>(find.byType(Image)).image, same(first));
      await tester.pumpWidget(render('file:///mnt/macos/avatar.png'));
      await tester.pumpAndSettle();
      expect(reads, ['/app/data/cache/avatar.png', '/mnt/macos/avatar.png']);
      expect(
        tester.widget<Image>(find.byType(Image)).image,
        isNot(same(first)),
      );
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'failed native VFS image displays an error affordance without framework file exception',
    (tester) async {
      final host = ComposeDslWebViewHostContext(
        packageName: 'test.images',
        routeInstanceId: 'route',
        executionContextKey: 'session',
        dispatchAction: (_, [payload]) async => null,
        runtimeOptionsProvider: () => {},
        readFileBytes: (_) async => throw StateError('VFS denied'),
      );
      await tester.pumpWidget(
        MaterialApp(
          home: buildComposeDslLayoutForTest(
            node: {
              'type': 'Image',
              'props': {'path': '/app/data/missing.png'},
              'children': [],
              'slots': {},
            },
            hostContext: host,
            splitMarkdownContent: (_) async => [],
            onTextInput: (_, text) async => null,
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.byIcon(Icons.broken_image_outlined), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );
}

/// Uses the actual binary bridge ABI and generated client decoder.
class _VfsFileBridge extends OperitRuntimeBridge {
  _VfsFileBridge(this.result, {this.success = true, this.error});
  final Map<String, Object?> result;
  final bool success;
  final String? error;
  final calls = <CoreCallRequest>[];

  @override
  Future<Uint8List> callBytes(CoreCallRequest request) async {
    calls.add(request);
    return encodeCoreLink([
      0,
      {
        'toolName': 'read_file_binary',
        'success': success,
        'result': result,
        'error': error,
      },
    ]);
  }

  @override
  Future<CorePushSink> push(CorePushRequest request) =>
      throw UnimplementedError();
  @override
  Future<CoreEvent> watchSnapshot(CoreWatchRequest request) =>
      throw UnimplementedError();
  @override
  Stream<CoreEvent> watchStream(CoreWatchRequest request) =>
      throw UnimplementedError();
}
