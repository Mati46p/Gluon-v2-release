/**
 * Gluon Extension Configuration
 *
 * Centralna konfiguracja dla różnych środowisk (dev/prod)
 */

// WAŻNE: Ustaw na false przed budowaniem wersji produkcyjnej!
export const DEV_MODE = true; 

// Poziomy logowania
export const LOG_LEVELS = {
  DEBUG: 0,  // Szczegółowe informacje debugowania
  INFO: 1,   // Ogólne informacje
  WARN: 2,   // Ostrzeżenia
  ERROR: 3,  // Błędy
  NONE: 999  // Wyłącz wszystkie logi
};

// Minimalny poziom logowania (wszystko poniżej zostanie zignorowane)
// W prod możesz ustawić na LOG_LEVELS.ERROR, aby zobaczyć tylko błędy
export const MIN_LOG_LEVEL = DEV_MODE ? LOG_LEVELS.DEBUG : LOG_LEVELS.ERROR;

// Dodatkowe opcje
export const CONFIG = {
  // WebSocket
  WEBSOCKET_URL: 'ws://127.0.0.1:8743',
  WEBSOCKET_RECONNECT_ATTEMPTS: 5,
  WEBSOCKET_RECONNECT_DELAY: 2000,

  // Timery
  LICENSE_CHECK_INTERVAL: 10000,
  FILE_TREE_POLLING_INTERVAL: 1000,
  REQUEST_TIMEOUT: 30000,

  // Logi
  ENABLE_EMOJI_IN_LOGS: DEV_MODE, // Emoji tylko w dev
  LOG_PREFIX_ENABLED: true,
};

export default {
  DEV_MODE,
  LOG_LEVELS,
  MIN_LOG_LEVEL,
  CONFIG
};