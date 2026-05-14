/**
 * Gluon Logger Module
 *
 * Centralizowany system logowania z obsługą środowisk dev/prod
 */

import { DEV_MODE, LOG_LEVELS, MIN_LOG_LEVEL, CONFIG } from './config.js';

/**
 * Sprawdza czy dany poziom logowania jest włączony
 */
function shouldLog(level) {
  return DEV_MODE && level >= MIN_LOG_LEVEL;
}

/**
 * Formatuje prefix dla wiadomości
 */
function formatPrefix(prefix, emoji = '') {
  if (!CONFIG.LOG_PREFIX_ENABLED) return '';

  const emojiPart = (CONFIG.ENABLE_EMOJI_IN_LOGS && emoji) ? `${emoji} ` : '';
  return prefix ? `[${prefix}] ${emojiPart}` : emojiPart;
}

/**
 * Logger class - główny interfejs logowania
 */
class Logger {
  constructor(prefix = 'Gluon') {
    this.prefix = prefix;
  }

  /**
   * Logi debugowania (DEBUG level)
   * Użyj dla szczegółowych informacji rozwojowych
   */
  debug(...args) {
    if (shouldLog(LOG_LEVELS.DEBUG)) {
      console.log(formatPrefix(this.prefix, '🔍'), ...args);
    }
  }

  /**
   * Standardowe logi informacyjne (INFO level)
   * Użyj dla ogólnych informacji o działaniu aplikacji
   */
  log(...args) {
    if (shouldLog(LOG_LEVELS.INFO)) {
      console.log(formatPrefix(this.prefix), ...args);
    }
  }

  /**
   * Logi informacyjne z emoji (INFO level)
   */
  info(...args) {
    if (shouldLog(LOG_LEVELS.INFO)) {
      console.log(formatPrefix(this.prefix, 'ℹ️'), ...args);
    }
  }

  /**
   * Logi sukcesu (INFO level)
   */
  success(...args) {
    if (shouldLog(LOG_LEVELS.INFO)) {
      console.log(formatPrefix(this.prefix, '✅'), ...args);
    }
  }

  /**
   * Ostrzeżenia (WARN level)
   * Zawsze używaj console.warn dla kompatybilności z dev tools
   */
  warn(...args) {
    if (shouldLog(LOG_LEVELS.WARN)) {
      console.warn(formatPrefix(this.prefix, '⚠️'), ...args);
    }
  }

  /**
   * Błędy (ERROR level)
   * Zawsze używaj console.error dla kompatybilności z dev tools
   * W prod domyślnie pokazuje tylko błędy
   */
  error(...args) {
    if (shouldLog(LOG_LEVELS.ERROR)) {
      console.error(formatPrefix(this.prefix, '❌'), ...args);
    }
  }

  /**
   * Logi grupowane (dla lepszej organizacji w konsoli)
   */
  group(label, collapsed = false) {
    if (shouldLog(LOG_LEVELS.DEBUG)) {
      if (collapsed) {
        console.groupCollapsed(formatPrefix(this.prefix), label);
      } else {
        console.group(formatPrefix(this.prefix), label);
      }
    }
  }

  groupEnd() {
    if (shouldLog(LOG_LEVELS.DEBUG)) {
      console.groupEnd();
    }
  }

  /**
   * Tabelki (przydatne do wyświetlania obiektów)
   */
  table(data) {
    if (shouldLog(LOG_LEVELS.DEBUG)) {
      console.log(formatPrefix(this.prefix));
      console.table(data);
    }
  }

  /**
   * Pomiar czasu wykonania
   */
  time(label) {
    if (shouldLog(LOG_LEVELS.DEBUG)) {
      console.time(`${formatPrefix(this.prefix)} ${label}`);
    }
  }

  timeEnd(label) {
    if (shouldLog(LOG_LEVELS.DEBUG)) {
      console.timeEnd(`${formatPrefix(this.prefix)} ${label}`);
    }
  }
}

// Eksportuj domyślny logger
export const logger = new Logger('Gluon');

// Eksportuj dedykowane loggery dla różnych modułów
export const backgroundLogger = new Logger('Gluon Background');
export const sidebarLogger = new Logger('Gluon Sidebar');
export const parserLogger = new Logger('Gluon Parser');
export const contextLogger = new Logger('Gluon Context');
export const fileTreeLogger = new Logger('Gluon FileTree');
export const templateLogger = new Logger('Gluon Template');

// Eksportuj funkcję do tworzenia custom loggerów
export function createLogger(prefix) {
  return new Logger(prefix);
}

// Default export
export default logger;