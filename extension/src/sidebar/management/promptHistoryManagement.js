// ============================================================================
// Prompt History Management Module
// Zarządza historią promptów dla każdego typu funkcji
// ============================================================================

const HISTORY_STORAGE_KEY = 'gluon_prompt_history';
const MAX_HISTORY_ITEMS = 20;

/**
 * Pobiera historię promptów dla danego typu
 */
export async function getPromptHistory(type) {
  const data = await chrome.storage.local.get(HISTORY_STORAGE_KEY);
  const allHistory = data[HISTORY_STORAGE_KEY] || {};
  return allHistory[type] || [];
}

/**
 * Zapisuje prompt do historii
 */
export async function savePromptToHistory(type, promptText) {
  if (!promptText || !promptText.trim()) return;

  const data = await chrome.storage.local.get(HISTORY_STORAGE_KEY);
  const allHistory = data[HISTORY_STORAGE_KEY] || {};
  let typeHistory = allHistory[type] || [];

  // Usuń duplikaty - jeśli ten sam prompt już istnieje, przenieś go na początek
  typeHistory = typeHistory.filter(item => item !== promptText.trim());

  // Dodaj nowy prompt na początek
  typeHistory.unshift(promptText.trim());

  // Ogranicz do MAX_HISTORY_ITEMS
  if (typeHistory.length > MAX_HISTORY_ITEMS) {
    typeHistory = typeHistory.slice(0, MAX_HISTORY_ITEMS);
  }

  allHistory[type] = typeHistory;
  await chrome.storage.local.set({ [HISTORY_STORAGE_KEY]: allHistory });
}

/**
 * Czyści historię dla danego typu
 */
export async function clearPromptHistory(type) {
  const data = await chrome.storage.local.get(HISTORY_STORAGE_KEY);
  const allHistory = data[HISTORY_STORAGE_KEY] || {};
  delete allHistory[type];
  await chrome.storage.local.set({ [HISTORY_STORAGE_KEY]: allHistory });
}

/**
 * Czyści całą historię promptów
 */
export async function clearAllPromptHistory() {
  await chrome.storage.local.set({ [HISTORY_STORAGE_KEY]: {} });
}

/**
 * Populuje dropdown z historią
 */
export async function populateHistoryDropdown(type, selectElement) {
  const history = await getPromptHistory(type);

  // Wyczyść istniejące opcje
  selectElement.innerHTML = '<option value="">-- Recent prompts --</option>';

  if (history.length === 0) {
    selectElement.disabled = true;
    return;
  }

  selectElement.disabled = false;

  history.forEach((prompt, index) => {
    const option = document.createElement('option');
    option.value = prompt;
    // Pokaż pierwsze 50 znaków jako podgląd
    const preview = prompt.length > 50 ? prompt.substring(0, 50) + '...' : prompt;
    option.textContent = `${index + 1}. ${preview}`;
    selectElement.appendChild(option);
  });
}

/**
 * Obsługuje nawigację strzałkami w textarea
 */
export class PromptHistoryNavigator {
  constructor(textarea, type) {
    this.textarea = textarea;
    this.type = type;
    this.history = [];
    this.currentIndex = -1;
    this.tempValue = '';
    this.isNavigating = false;
  }

  async init() {
    this.history = await getPromptHistory(this.type);
    this.currentIndex = -1;
    this.tempValue = '';
  }

  handleKeyDown(event) {
    // Obsługuj tylko strzałki góra/dół
    const isUp = event.key === 'ArrowUp';
    const isDown = event.key === 'ArrowDown';

    if (isUp || isDown) {
      event.preventDefault();

      if (this.history.length === 0) return;

      // Zapisz aktualną wartość jeśli to pierwsza nawigacja
      if (!this.isNavigating) {
        this.tempValue = this.textarea.value;
        this.isNavigating = true;
      }

      if (isUp) {
        // Góra - starsze wpisy
        if (this.currentIndex < this.history.length - 1) {
          this.currentIndex++;
          this.textarea.value = this.history[this.currentIndex];
        }
      } else {
        // Dół - nowsze wpisy
        if (this.currentIndex > -1) {
          this.currentIndex--;
          if (this.currentIndex === -1) {
            // Powrót do oryginalnej wartości
            this.textarea.value = this.tempValue;
            this.isNavigating = false;
          } else {
            this.textarea.value = this.history[this.currentIndex];
          }
        }
      }
    }
  }

  reset() {
    this.currentIndex = -1;
    this.tempValue = '';
    this.isNavigating = false;
  }
}
