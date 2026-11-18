/**
 * 统一错误码系统
 *
 * 提供跨平台的错误码定义，用于 Rust 和 JavaScript 之间的错误通信。
 *
 * 错误码范围:
 * - 1000-1999: 通用错误
 * - 2000-2999: 插件错误
 * - 3000-3999: 配置错误
 * - 4000-4999: 网络错误
 * - 5000-5999: 文件系统错误
 *
 * @module error-codes
 */

/**
 * 错误码枚举
 * @readonly
 * @enum {number}
 */
export const ErrorCode = {
  // === 通用错误 (1000-1999) ===
  /** 未知错误 */
  UNKNOWN: 1000,
  /** 无效参数 */
  INVALID_ARGUMENT: 1001,
  /** 资源未找到 */
  NOT_FOUND: 1002,
  /** 资源已存在 */
  ALREADY_EXISTS: 1003,
  /** 权限不足 */
  PERMISSION_DENIED: 1004,
  /** 操作超时 */
  TIMEOUT: 1005,
  /** 已取消 */
  CANCELLED: 1006,
  /** 未实现 */
  NOT_IMPLEMENTED: 1007,
  /** 内部错误 */
  INTERNAL: 1008,

  // === 插件错误 (2000-2999) ===
  /** 插件未找到 */
  PLUGIN_NOT_FOUND: 2000,
  /** 插件加载失败 */
  PLUGIN_LOAD_FAILED: 2001,
  /** 插件依赖缺失 */
  PLUGIN_DEPENDENCY_MISSING: 2002,
  /** 插件版本不兼容 */
  PLUGIN_VERSION_INCOMPATIBLE: 2003,
  /** 插件初始化失败 */
  PLUGIN_INIT_FAILED: 2004,
  /** 插件卸载失败 */
  PLUGIN_UNLOAD_FAILED: 2005,
  /** 插件已加载 */
  PLUGIN_ALREADY_LOADED: 2006,
  /** 插件热重载失败 */
  PLUGIN_RELOAD_FAILED: 2007,

  // === 配置错误 (3000-3999) ===
  /** 配置无效 */
  CONFIG_INVALID: 3000,
  /** 配置缺失 */
  CONFIG_MISSING: 3001,
  /** 配置解析失败 */
  CONFIG_PARSE_FAILED: 3002,
  /** 配置键未找到 */
  CONFIG_KEY_NOT_FOUND: 3003,
  /** 配置类型错误 */
  CONFIG_TYPE_MISMATCH: 3004,

  // === 网络错误 (4000-4999) ===
  /** 连接失败 */
  CONNECTION_FAILED: 4000,
  /** 连接超时 */
  CONNECTION_TIMEOUT: 4001,
  /** 请求失败 */
  REQUEST_FAILED: 4002,
  /** 响应无效 */
  INVALID_RESPONSE: 4003,
  /** DNS 解析失败 */
  DNS_RESOLUTION_FAILED: 4004,
  /** SSL/TLS 错误 */
  TLS_ERROR: 4005,

  // === 文件系统错误 (5000-5999) ===
  /** 文件未找到 */
  FILE_NOT_FOUND: 5000,
  /** 文件已存在 */
  FILE_ALREADY_EXISTS: 5001,
  /** 目录未找到 */
  DIRECTORY_NOT_FOUND: 5002,
  /** 路径无效 */
  INVALID_PATH: 5003,
  /** 读取失败 */
  READ_FAILED: 5004,
  /** 写入失败 */
  WRITE_FAILED: 5005,
  /** 删除失败 */
  DELETE_FAILED: 5006,
  /** 文件权限不足 */
  FILE_PERMISSION_DENIED: 5007,
};

/**
 * 错误码描述
 * @type {Object<number, string>}
 */
export const ErrorCodeDescription = {
  [ErrorCode.UNKNOWN]: '未知错误',
  [ErrorCode.INVALID_ARGUMENT]: '无效参数',
  [ErrorCode.NOT_FOUND]: '资源未找到',
  [ErrorCode.ALREADY_EXISTS]: '资源已存在',
  [ErrorCode.PERMISSION_DENIED]: '权限不足',
  [ErrorCode.TIMEOUT]: '操作超时',
  [ErrorCode.CANCELLED]: '已取消',
  [ErrorCode.NOT_IMPLEMENTED]: '未实现',
  [ErrorCode.INTERNAL]: '内部错误',

  [ErrorCode.PLUGIN_NOT_FOUND]: '插件未找到',
  [ErrorCode.PLUGIN_LOAD_FAILED]: '插件加载失败',
  [ErrorCode.PLUGIN_DEPENDENCY_MISSING]: '插件依赖缺失',
  [ErrorCode.PLUGIN_VERSION_INCOMPATIBLE]: '插件版本不兼容',
  [ErrorCode.PLUGIN_INIT_FAILED]: '插件初始化失败',
  [ErrorCode.PLUGIN_UNLOAD_FAILED]: '插件卸载失败',
  [ErrorCode.PLUGIN_ALREADY_LOADED]: '插件已加载',
  [ErrorCode.PLUGIN_RELOAD_FAILED]: '插件热重载失败',

  [ErrorCode.CONFIG_INVALID]: '配置无效',
  [ErrorCode.CONFIG_MISSING]: '配置缺失',
  [ErrorCode.CONFIG_PARSE_FAILED]: '配置解析失败',
  [ErrorCode.CONFIG_KEY_NOT_FOUND]: '配置键未找到',
  [ErrorCode.CONFIG_TYPE_MISMATCH]: '配置类型错误',

  [ErrorCode.CONNECTION_FAILED]: '连接失败',
  [ErrorCode.CONNECTION_TIMEOUT]: '连接超时',
  [ErrorCode.REQUEST_FAILED]: '请求失败',
  [ErrorCode.INVALID_RESPONSE]: '响应无效',
  [ErrorCode.DNS_RESOLUTION_FAILED]: 'DNS 解析失败',
  [ErrorCode.TLS_ERROR]: 'SSL/TLS 错误',

  [ErrorCode.FILE_NOT_FOUND]: '文件未找到',
  [ErrorCode.FILE_ALREADY_EXISTS]: '文件已存在',
  [ErrorCode.DIRECTORY_NOT_FOUND]: '目录未找到',
  [ErrorCode.INVALID_PATH]: '路径无效',
  [ErrorCode.READ_FAILED]: '读取失败',
  [ErrorCode.WRITE_FAILED]: '写入失败',
  [ErrorCode.DELETE_FAILED]: '删除失败',
  [ErrorCode.FILE_PERMISSION_DENIED]: '文件权限不足',
};

/**
 * 应用程序错误
 */
export class AppError extends Error {
  /**
   * 创建新的错误
   * @param {number} code - 错误码
   * @param {string} message - 错误消息
   * @param {Object} [context] - 上下文信息
   */
  constructor(code, message, context = null) {
    super(message);
    this.name = 'AppError';
    /** @type {number} */
    this.code = code;
    /** @type {string} */
    this.message = message;
    /** @type {Object|null} */
    this.context = context;
  }

  /**
   * 获取错误码描述
   * @returns {string}
   */
  description() {
    return ErrorCodeDescription[this.code] || '未知错误';
  }

  /**
   * 转换为 JSON
   * @returns {Object}
   */
  toJSON() {
    return {
      code: this.code,
      message: this.message,
      context: this.context,
    };
  }

  /**
   * 从 JSON 创建
   * @param {Object} json - JSON 对象
   * @returns {AppError}
   */
  static fromJSON(json) {
    return new AppError(json.code, json.message, json.context);
  }

  /**
   * 创建带上下文的错误
   * @param {number} code - 错误码
   * @param {string} message - 错误消息
   * @param {Object} context - 上下文信息
   * @returns {AppError}
   */
  static withContext(code, message, context) {
    return new AppError(code, message, context);
  }
}

/**
 * 检查错误码是否有效
 * @param {number} code - 错误码
 * @returns {boolean}
 */
export function isValidErrorCode(code) {
  return Object.values(ErrorCode).includes(code);
}

/**
 * 获取错误码的描述
 * @param {number} code - 错误码
 * @returns {string}
 */
export function getErrorCodeDescription(code) {
  return ErrorCodeDescription[code] || '未知错误';
}

/**
 * 从错误码创建错误
 * @param {number} code - 错误码
 * @param {string} [message] - 错误消息（可选，默认使用描述）
 * @returns {AppError}
 */
export function createError(code, message = null) {
  return new AppError(code, message || getErrorCodeDescription(code));
}

export default {
  ErrorCode,
  ErrorCodeDescription,
  AppError,
  isValidErrorCode,
  getErrorCodeDescription,
  createError,
};
