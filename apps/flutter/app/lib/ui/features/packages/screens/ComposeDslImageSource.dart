// ignore_for_file: file_names

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/painting.dart';

import '../../../../core/proxy/generated/CoreProxyClients.g.dart';
import '../../../../core/proxy/generated/CoreProxyModels.g.dart' as core_proxy;

/// Reads virtual files through the actual Core file service, never dart:io.
Future<Uint8List> readComposeDslVfsBytes(
  GeneratedCoreProxyClients clients,
  String path,
) async {
  final result = await clients.application.aiToolHandler().executeTool(
    tool: core_proxy.CoreOperitToolsToolExecutionManagerAiTool(
      name: 'read_file_binary',
      parameters: [
        core_proxy.CoreOperitToolsToolExecutionManagerToolParameter(
          name: 'path',
          value: path,
        ),
      ],
    ),
  );
  if (!result.success) {
    throw StateError('DSL VFS image $path: ${result.error}');
  }
  final data = result.result.value;
  if (data is! core_proxy.BinaryFileContentData) {
    throw StateError('DSL VFS image did not return binary file content');
  }
  return base64Decode(data.contentBase64);
}

/// Gives known VFS roots precedence even when wrapped in a file URI.
String? composeDslVfsImagePath(String source) {
  final uri = Uri.tryParse(source);
  if (uri?.scheme == 'vfs' || uri?.scheme == 'operit-vfs') {
    final host = uri!.host;
    return host.isEmpty || host == 'localhost' ? uri.path : '/$host${uri.path}';
  }
  final path = uri?.scheme == 'file' ? uri!.toFilePath() : source;
  if (path == '/app' ||
      path.startsWith('/app/') ||
      path == '/mnt' ||
      path.startsWith('/mnt/') ||
      path.startsWith('runtime/'))
    return path;
  return null;
}

/// Resolves VFS before native files. A VFS failure must not fall back to the OS.
Future<ImageProvider> resolveComposeDslImage(
  String source, {
  required Future<Uint8List> Function(String path) readVfsBytes,
}) async {
  final vfs = composeDslVfsImagePath(source);
  if (vfs != null) return MemoryImage(await readVfsBytes(vfs));
  final uri = Uri.tryParse(source);
  return switch (uri?.scheme) {
    'data' => MemoryImage(UriData.parse(source).contentAsBytes()),
    'file' => FileImage(File.fromUri(uri!)),
    _ =>
      source.startsWith('/') ? FileImage(File(source)) : NetworkImage(source),
  };
}
