/**
 * 插件市场 - 插件注册表
 *
 * 管理插件的注册、版本控制和依赖解析。
 *
 * @module plugin-registry
 */

import { ErrorCode, AppError } from './error-codes.js';

/**
 * 插件信息
 * @typedef {Object} PluginInfo
 * @property {string} name - 插件名称
 * @property {string} version - 版本号
 * @property {string} description - 描述
 * @property {string} author - 作者
 * @property {string[]} dependencies - 依赖列表
 * @property {string} downloadUrl - 下载地址
 * @property {number} downloadCount - 下载次数
 * @property {Date} publishDate - 发布日期
 * @property {Date} updateDate - 更新日期
 */

/**
 * 插件注册表
 */
export class PluginRegistry {
  constructor() {
    /** @type {Map<string, PluginInfo[]>} */
    this.plugins = new Map();
  }

  /**
   * 注册插件
   * @param {PluginInfo} plugin - 插件信息
   * @throws {AppError} 如果插件已存在
   */
  register(plugin) {
    const existing = this.plugins.get(plugin.name) || [];

    // 检查版本是否已存在
    if (existing.some(p => p.version === plugin.version)) {
      throw new AppError(
        ErrorCode.PLUGIN_ALREADY_LOADED,
        `Plugin '${plugin.name}' version '${plugin.version}' already registered`
      );
    }

    existing.push(plugin);
    this.plugins.set(plugin.name, existing);
  }

  /**
   * 获取插件的最新版本
   * @param {string} name - 插件名称
   * @returns {PluginInfo|null}
   */
  getLatest(name) {
    const versions = this.plugins.get(name);
    if (!versions || versions.length === 0) {
      return null;
    }

    // 按版本号排序，返回最新的
    return versions.sort((a, b) => this.compareVersions(b.version, a.version))[0];
  }

  /**
   * 获取插件的特定版本
   * @param {string} name - 插件名称
   * @param {string} version - 版本号
   * @returns {PluginInfo|null}
   */
  getVersion(name, version) {
    const versions = this.plugins.get(name);
    if (!versions) {
      return null;
    }

    return versions.find(p => p.version === version) || null;
  }

  /**
   * 检查插件是否已注册
   * @param {string} name - 插件名称
   * @returns {boolean}
   */
  has(name) {
    return this.plugins.has(name);
  }

  /**
   * 获取所有插件
   * @returns {PluginInfo[]}
   */
  listAll() {
    const all = [];
    for (const versions of this.plugins.values()) {
      all.push(...versions);
    }
    return all;
  }

  /**
   * 获取插件的所有版本
   * @param {string} name - 插件名称
   * @returns {PluginInfo[]}
   */
  getVersions(name) {
    return this.plugins.get(name) || [];
  }

  /**
   * 解析依赖
   * @param {string} name - 插件名称
   * @param {string} version - 版本号
   * @returns {PluginInfo[]} 依赖列表
   * @throws {AppError} 如果依赖缺失
   */
  resolveDependencies(name, version) {
    const plugin = this.getVersion(name, version);
    if (!plugin) {
      throw new AppError(
        ErrorCode.PLUGIN_NOT_FOUND,
        `Plugin '${name}' version '${version}' not found`
      );
    }

    const resolved = [];
    for (const depName of plugin.dependencies) {
      const dep = this.getLatest(depName);
      if (!dep) {
        throw new AppError(
          ErrorCode.PLUGIN_DEPENDENCY_MISSING,
          `Dependency '${depName}' required by '${name}' not found`
        );
      }
      resolved.push(dep);
    }

    return resolved;
  }

  /**
   * 比较版本号
   * @param {string} v1 - 版本 1
   * @param {string} v2 - 版本 2
   * @returns {number} -1, 0, 或 1
   */
  compareVersions(v1, v2) {
    const parts1 = v1.split('.').map(Number);
    const parts2 = v2.split('.').map(Number);
    const len = Math.max(parts1.length, parts2.length);

    for (let i = 0; i < len; i++) {
      const p1 = parts1[i] || 0;
      const p2 = parts2[i] || 0;

      if (p1 > p2) return 1;
      if (p1 < p2) return -1;
    }

    return 0;
  }

  /**
   * 增加下载次数
   * @param {string} name - 插件名称
   * @param {string} version - 版本号
   */
  incrementDownloads(name, version) {
    const plugin = this.getVersion(name, version);
    if (plugin) {
      plugin.downloadCount++;
    }
  }

  /**
   * 清空注册表
   */
  clear() {
    this.plugins.clear();
  }
}

/**
 * 插件发布器
 */
export class PluginPublisher {
  constructor(registry) {
    this.registry = registry;
  }

  /**
   * 发布插件
   * @param {PluginInfo} plugin - 插件信息
   * @returns {PluginInfo}
   */
  publish(plugin) {
    // 验证插件信息
    this.validatePlugin(plugin);

    // 注册插件
    this.registry.register(plugin);

    return plugin;
  }

  /**
   * 验证插件信息
   * @param {PluginInfo} plugin - 插件信息
   * @throws {AppError} 如果验证失败
   */
  validatePlugin(plugin) {
    if (!plugin.name || plugin.name.trim() === '') {
      throw new AppError(
        ErrorCode.INVALID_ARGUMENT,
        'Plugin name is required'
      );
    }

    if (!plugin.version || !/^\d+\.\d+\.\d+$/.test(plugin.version)) {
      throw new AppError(
        ErrorCode.INVALID_ARGUMENT,
        'Plugin version must be in semver format (e.g., 1.0.0)'
      );
    }

    if (!plugin.description) {
      throw new AppError(
        ErrorCode.INVALID_ARGUMENT,
        'Plugin description is required'
      );
    }

    if (!plugin.downloadUrl) {
      throw new AppError(
        ErrorCode.INVALID_ARGUMENT,
        'Plugin download URL is required'
      );
    }
  }
}

export default {
  PluginRegistry,
  PluginPublisher,
};
