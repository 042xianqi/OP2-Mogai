// ignore_for_file: file_names

import 'dart:convert';
import 'dart:typed_data';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/services.dart';
import 'package:operit_ui/operit_ui.dart';

import '../../core/proxy/generated/CoreProxyClients.g.dart';
import '../../core/proxy/generated/CoreProxyModels.g.dart' as core_proxy;
import '../../core/host/SelectedFileInput.dart';

const int themeArchiveMaxBytes = 64 * 1024 * 1024;

/// Bounded system-document input; no Dart filesystem or HTTP implementation.
class ThemeArchiveInput {
  const ThemeArchiveInput(this.name, this.bytes, this.validation);

  final String name;
  final Uint8List bytes;
  final core_proxy.ThemeValidationReceipt validation;
}

class ThemeLibraryService {
  ThemeLibraryService(this.clients);

  final GeneratedCoreProxyClients clients;
  final ThemeFontRegistry _fonts = ThemeFontRegistry();
  Future<BuiltinThemeFonts>? _builtinFonts;
  final Map<String, Uint8List> _archiveBytesCache = <String, Uint8List>{};
  final Map<String, Uint8List?> _backgroundThumbnailCache =
      <String, Uint8List?>{};
  final Map<String, Future<Uint8List?>> _backgroundThumbnailFutures =
      <String, Future<Uint8List?>>{};

  void _cacheArchiveBytes(String sha256, Uint8List bytes) {
    _archiveBytesCache.remove(sha256);
    if (_archiveBytesCache.length >= 2) {
      _archiveBytesCache.remove(_archiveBytesCache.keys.first);
    }
    _archiveBytesCache[sha256] = bytes;
  }

  /// Returns cached custom background image bytes for [archiveSha256] if already loaded.
  Uint8List? cachedBackgroundImageBytes(String themeId, String archiveSha256) =>
      _backgroundThumbnailCache['$themeId@$archiveSha256'];

  /// Reports whether custom background image bytes for [archiveSha256] have been resolved.
  bool hasResolvedBackgroundImage(String themeId, String archiveSha256) =>
      _backgroundThumbnailCache.containsKey('$themeId@$archiveSha256');

  /// Reads and caches the custom background image declared in theme.json
  /// (background.asset) of the specified theme template.
  Future<Uint8List?> readBackgroundImageBytes({
    required String themeId,
    required String archiveSha256,
  }) {
    final templateKey = '$themeId@$archiveSha256';
    if (_backgroundThumbnailCache.containsKey(templateKey)) {
      return Future<Uint8List?>.value(_backgroundThumbnailCache[templateKey]);
    }
    return _backgroundThumbnailFutures.putIfAbsent(templateKey, () async {
      try {
        var bytes = _archiveBytesCache[archiveSha256];
        if (bytes == null) {
          bytes = await clients.application.readToolPkgForThemeTemplate(
            themeId: themeId,
            archiveSha256: archiveSha256,
          );
          _cacheArchiveBytes(archiveSha256, bytes);
        }
        final payload = await clients.application.previewThemeArchive(
          archiveBytes: bytes,
          themeId: themeId,
        );
        if (payload.validation.archiveSha256 != archiveSha256 ||
            payload.themeId != themeId) {
          throw StateError('主题背景缩略图回执与所选包不一致');
        }
        final rawTheme = jsonDecode(payload.themeJson) as Map<String, dynamic>;
        final background = rawTheme['background'] as Map<String, dynamic>?;
        final assetPath = background?['asset'] as String?;
        if (assetPath == null || assetPath.isEmpty) {
          _backgroundThumbnailCache[templateKey] = null;
          return null;
        }
        for (final asset in payload.assets) {
          if (asset.path == assetPath) {
            final view = asset.bytes.asUnmodifiableView();
            _backgroundThumbnailCache[templateKey] = view;
            return view;
          }
        }
        _backgroundThumbnailCache[templateKey] = null;
        return null;
      } finally {
        _backgroundThumbnailFutures.remove(templateKey);
      }
    });
  }

  /// Reads a ToolPkg and asks the caller to choose among its declared templates.
  Future<ThemeArchiveInput?> inspectPickedArchive({
    required Future<String?> Function(List<core_proxy.ThemeValidationReceipt>)
    selectTemplate,
  }) async {
    final file = await SelectedFileInput.pickSnapshot(
      acceptedTypeGroups: const <XTypeGroup>[
        XTypeGroup(label: 'Operit theme', extensions: <String>['toolpkg']),
      ],
    );
    if (file == null) return null;
    try {
      if (!file.name.toLowerCase().endsWith('.toolpkg')) {
        throw FormatException('请选择 .toolpkg ToolPkg');
      }
      final byteLength = file.byteLength;
      if (byteLength == null ||
          byteLength <= 0 ||
          byteLength > themeArchiveMaxBytes) {
        throw FormatException('ToolPkg必须为 1 字节至 64 MiB，且文件大小必须可读取');
      }
      final builder = BytesBuilder(copy: false);
      await for (final chunk in file.chunks()) {
        if (builder.length + chunk.length > byteLength ||
            builder.length + chunk.length > themeArchiveMaxBytes) {
          throw FormatException('ToolPkg流长度超过声明大小');
        }
        builder.add(chunk);
      }
      final bytes = builder.takeBytes();
      if (bytes.length != byteLength) {
        throw FormatException('ToolPkg流长度与声明大小不一致');
      }
      final templates = await clients.application.inspectThemeArchive(
        archiveBytes: bytes,
      );
      if (templates.isEmpty) throw FormatException('此 ToolPkg 没有主题模板');
      final selectedId = templates.length == 1
          ? templates.single.themeId
          : await selectTemplate(templates);
      if (selectedId == null) return null;
      final validation = templates.singleWhere(
        (template) => template.themeId == selectedId,
      );
      final view = bytes.asUnmodifiableView();
      _cacheArchiveBytes(validation.archiveSha256, view);
      return ThemeArchiveInput(file.name, view, validation);
    } finally {
      await file.close();
    }
  }

  /// Removes a theme package from the Space-owned library and clears its byte cache.
  Future<void> deleteTheme(String themeId, String archiveSha256) async {
    await clients.application.deleteTheme(
      themeId: themeId,
      archiveSha256: archiveSha256,
    );
    _archiveBytesCache.remove(archiveSha256);
    _backgroundThumbnailCache.removeWhere(
      (key, _) => key.endsWith('@$archiveSha256'),
    );
    _backgroundThumbnailFutures.removeWhere(
      (key, _) => key.endsWith('@$archiveSha256'),
    );
  }

  /// Core revalidates the sealed archive. This does not prepare or apply it.
  Future<core_proxy.ThemeInstallReceipt> importArchive(
    ThemeArchiveInput input,
  ) async {
    final alreadyPresent = await clients.application.importToolPkgArchive(
      archiveBytes: input.bytes,
      expectedArchiveBytes: input.validation.archiveBytes,
      expectedArchiveSha256: input.validation.archiveSha256,
    );
    final entries = await clients.application.listThemes();
    final entry = entries.singleWhere(
      (entry) =>
          entry.themeId == input.validation.themeId &&
          entry.archiveSha256 == input.validation.archiveSha256,
    );
    final receipt = core_proxy.ThemeInstallReceipt(
      schemaVersion: entry.schemaVersion,
      status: alreadyPresent
          ? core_proxy.ThemeInstallStatus.alreadyPresent
          : core_proxy.ThemeInstallStatus.imported,
      entry: entry,
      selection: null,
      selectionChanged: false,
      validation: entry.validation,
    );
    if (receipt.selectionChanged ||
        receipt.selection != null ||
        (receipt.status != core_proxy.ThemeInstallStatus.imported &&
            receipt.status != core_proxy.ThemeInstallStatus.alreadyPresent) ||
        receipt.validation.archiveSha256 != input.validation.archiveSha256 ||
        receipt.validation.archiveBytes != input.bytes.length) {
      throw StateError('主题导入回执与已确认包不一致');
    }
    return receipt;
  }

  /// Every use, including restored node choices, re-enters the Rust validator.
  Future<PreparedThemeVisual> prepare(
    String themeId,
    String archiveSha256, {
    String? expectedVersion,
  }) async {
    var bytes = _archiveBytesCache[archiveSha256];
    if (bytes == null) {
      bytes = await clients.application.readToolPkgForThemeTemplate(
        themeId: themeId,
        archiveSha256: archiveSha256,
      );
      _cacheArchiveBytes(archiveSha256, bytes);
    }
    final payload = await clients.application.previewThemeArchive(
      archiveBytes: bytes,
      themeId: themeId,
    );
    if (payload.validation.archiveSha256 != archiveSha256 ||
        payload.validation.archiveBytes != bytes.length ||
        payload.validation.validatorVersion != 1 ||
        payload.themeId != themeId ||
        (expectedVersion != null && payload.version != expectedVersion)) {
      throw StateError('主题预览回执与所选包不一致');
    }
    final expected = <String, String>{
      for (final file in payload.validation.files)
        if (file.path == 'preview.png' || file.path.startsWith('assets/'))
          file.path: file.sha256,
    };
    final assets = <String, Uint8List>{};
    for (final asset in payload.assets) {
      if (assets.containsKey(asset.path) || !expected.containsKey(asset.path)) {
        throw StateError('主题预览资源重复或未登记：${asset.path}');
      }
      assets[asset.path] = asset.bytes;
    }
    if (assets.length != expected.length) throw StateError('主题预览资源不完整');
    final rawThemeMap = jsonDecode(payload.themeJson) as Map<String, dynamic>;
    final bgAssetPath =
        (rawThemeMap['background'] as Map<String, dynamic>?)?['asset']
            as String?;
    _backgroundThumbnailCache['$themeId@$archiveSha256'] = bgAssetPath != null
        ? assets[bgAssetPath]?.asUnmodifiableView()
        : null;
    final spec = ThemeVisualSpecV1.fromCheckedContract(rawThemeMap);
    final builtinFonts = await (_builtinFonts ??= BuiltinThemeFonts.load(
      bundle: rootBundle,
    ));
    return PreparedThemeVisual.prepare(
      spec: spec,
      assets: assets,
      assetSha256: expected,
      builtinFonts: builtinFonts,
      fontRegistry: _fonts,
    );
  }
}
