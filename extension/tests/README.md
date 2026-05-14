# Gluon Extension - Test Suite

Kompleksowy zestaw testów jednostkowych dla workflow-manager.js

## 📋 Spis Treści

- [Instalacja](#instalacja)
- [Uruchomienie Testów](#uruchomienie-testów)
- [Struktura Testów](#struktura-testów)
- [Pokrycie Kodu](#pokrycie-kodu)
- [Wykryte Słabe Punkty](#wykryte-słabe-punkty)
- [Przykłady](#przykłady)

## 🚀 Instalacja

### Wymagania

- Node.js >= 18.x
- npm >= 9.x

### Kroki instalacji

```bash
cd extension
npm install
```

Zostaną zainstalowane następujące paczki:
- `jest` - framework testowy
- `@jest/globals` - globale dla testów
- `jest-environment-jsdom` - środowisko DOM dla testów
- `jest-chrome` - mocki dla Chrome Extension API

## ▶️ Uruchomienie Testów

### Wszystkie testy

```bash
npm test
```

### Tryb watch (automatyczne uruchamianie przy zmianach)

```bash
npm run test:watch
```

### Testy z pokryciem kodu

```bash
npm run test:coverage
```

Wynik zostanie zapisany w katalogu `coverage/` oraz wyświetlony w konsoli.

### Tylko testy jednostkowe

```bash
npm run test:unit
```

### Pojedynczy plik testowy

```bash
npm test -- workflow-manager.sendWorkflowMessage.test.js
```

### Testy z filtrem nazwy

```bash
npm test -- --testNamePattern="should timeout"
```

## 📁 Struktura Testów

```
extension/
├── tests/
│   ├── setup.js                                      # Konfiguracja globalna
│   └── unit/
│       ├── workflow-manager.sendWorkflowMessage.test.js    # Testy komunikacji
│       ├── workflow-manager.tabManagement.test.js          # Testy zarządzania zakładkami
│       ├── workflow-manager.validation.test.js             # Testy walidacji
│       └── workflow-manager.importExport.test.js           # Testy import/export
├── jest.config.js                                   # Konfiguracja Jest
└── package.json                                     # Skrypty i zależności
```

## 📊 Pokrycie Kodu

### Aktualny cel: 60% pokrycia dla Priorytetu 1

### Obszary testowane:

#### 1. **State Management** (sendWorkflowMessage.test.js)
- ✅ `sendWorkflowMessage()` - komunikacja z backendem
- ✅ Obsługa timeout (15 sekund)
- ✅ Zarządzanie `pendingRequests`
- ✅ Race conditions i memory leaks
- ✅ Obsługa błędów Chrome Runtime API

**Liczba testów:** 31
**Pokrycie:** ~95% dla `sendWorkflowMessage()` i powiązanych metod

#### 2. **Tab Management** (tabManagement.test.js)
- ✅ `saveCurrentTabWorkflow()` - zapis stanu workflow
- ✅ `createNewTab()` - tworzenie zakładek
- ✅ `switchToTab()` - przełączanie zakładek
- ✅ `closeTab()` - zamykanie zakładek
- ✅ `duplicateTab()` - duplikowanie zakładek
- ✅ `renameTab()` - zmiana nazwy
- ✅ Persystencja w localStorage
- ✅ Deep copy i izolacja stanów

**Liczba testów:** 52
**Pokrycie:** ~90% dla zarządzania zakładkami

#### 3. **Validation Logic** (validation.test.js)
- ✅ `handleAddAgent()` - walidacja inputów
- ✅ `connectionExists()` - detekcja duplikatów połączeń
- ✅ `escapeHtml()` - ochrona przed XSS
- ✅ Walidacja nazw agentów
- ✅ Walidacja szablonów wrapper
- ✅ Walidacja typów agentów

**Liczba testów:** 68
**Pokrycie:** ~85% dla logiki walidacji

#### 4. **Import/Export** (importExport.test.js)
- ✅ `exportCurrentTab()` - eksport do JSON
- ✅ `importTabFromFile()` - import z JSON
- ✅ Walidacja struktury plików
- ✅ Obsługa błędnych danych
- ✅ Round-trip (eksport → import)
- ✅ Edge cases (BOM, Unicode, duże pliki)

**Liczba testów:** 35
**Pokrycie:** ~80% dla import/export

## 🔍 Wykryte Słabe Punkty

Poniżej znajdują się zidentyfikowane problemy w kodzie, które są testowane:

### 🔴 Krytyczne

1. **Race Conditions w pendingRequests**
   - **Problem:** Timeout może nie usuwać promise z mapy
   - **Test:** `workflow-manager.sendWorkflowMessage.test.js:170-205`
   - **Rekomendacja:** Dodać mechanizm cleanup w finally block

2. **Memory Leaks w renderAgents()**
   - **Problem:** Event listenery dodawane bez cleanup przy re-renderze
   - **Test:** Brak testu (wymaga integracji z DOM)
   - **Rekomendacja:** Użyć event delegation

3. **XSS w renderAgentCard()**
   - **Problem:** Użycie innerHTML z częściową sanityzacją
   - **Test:** `workflow-manager.validation.test.js:460-640`
   - **Rekomendacja:** Użyć textContent lub DOMPurify

4. **Brak Obsługi Błędów w Async Operations**
   - **Problem:** Niektóre metody async nie mają try-catch
   - **Test:** `workflow-manager.validation.test.js:320-380`
   - **Rekomendacja:** Dodać globalne error handling

### 🟡 Wysokie

5. **Deep Copy przez JSON.parse(JSON.stringify())**
   - **Problem:** Nie obsługuje cyklicznych referencji
   - **Test:** `workflow-manager.tabManagement.test.js:140-150`
   - **Rekomendacja:** Użyć biblioteki jak lodash.cloneDeep

6. **Synchroniczne localStorage Operations**
   - **Problem:** Blokowanie UI thread
   - **Test:** `workflow-manager.tabManagement.test.js:520-540`
   - **Rekomendacja:** Debounce i async storage API

7. **Brak Walidacji Rozmiaru Pliku w Import**
   - **Problem:** Możliwość zablokowania przez duże pliki
   - **Test:** `workflow-manager.importExport.test.js:450-470`
   - **Rekomendacja:** Dodać limit rozmiaru (np. 10MB)

## 🧪 Przykłady

### Uruchomienie specyficznego testu

```bash
# Test tylko timeoutów
npm test -- --testNamePattern="timeout"

# Test tylko walidacji XSS
npm test -- --testNamePattern="XSS"

# Test tylko zarządzania zakładkami
npm test -- tabManagement.test.js
```

### Debugging testów

```bash
# Uruchom z verbose output
npm test -- --verbose

# Uruchom pojedynczy test
npm test -- --testNamePattern="should timeout after 15 seconds"

# Uruchom z node debugger
node --inspect-brk node_modules/.bin/jest --runInBand
```

### Analiza pokrycia

Po uruchomieniu `npm run test:coverage` sprawdź:

```bash
# Otwórz raport HTML
start coverage/lcov-report/index.html  # Windows
open coverage/lcov-report/index.html   # macOS
xdg-open coverage/lcov-report/index.html  # Linux
```

## 📝 Pisanie Nowych Testów

### Przykład testu

```javascript
import { describe, test, expect, beforeEach } from '@jest/globals';

describe('MyFeature', () => {
  let manager;

  beforeEach(() => {
    manager = new TestableWorkflowManager();
  });

  test('should do something', () => {
    const result = manager.myMethod();
    expect(result).toBe(true);
  });

  test('should handle error', async () => {
    await expect(manager.myAsyncMethod()).rejects.toThrow('Error message');
  });
});
```

### Best Practices

1. **Izolacja testów** - każdy test powinien być niezależny
2. **Nazwy testów** - jasno opisuj co test sprawdza
3. **Arrange-Act-Assert** - struktura testu: przygotuj → wykonaj → sprawdź
4. **Mock'uj zależności** - izoluj testowaną jednostkę
5. **Test edge cases** - null, undefined, puste tablice, etc.

## 🐛 Znane Problemy

### Node.js ESM Support

Jeśli wystąpią problemy z importami ES Modules:

```bash
# Upewnij się, że używasz node --experimental-vm-modules
node --experimental-vm-modules node_modules/jest/bin/jest.js
```

### Chrome Extension API Mocks

Wszystkie API Chrome są mockowane w `tests/setup.js`. Jeśli potrzebujesz dodatkowych mocków:

```javascript
// W pliku testu
beforeEach(() => {
  chrome.storage.local.get.mockImplementation((keys, callback) => {
    callback({ key: 'value' });
  });
});
```

## 📈 Metrics

### Aktualne Statystyki

- **Całkowita liczba testów:** 186
- **Średni czas wykonania:** ~2-3 sekundy
- **Pokrycie kodu:** ~60% (cel Priorytetu 1)
- **Testy przechodzące:** 186/186 ✅

### Cel Pokrycia

```
Priorytet 1 (Aktualny):  60% ████████░░  [State Management, Validation, Tabs]
Priorytet 2 (Następny):  25% ███░░░░░░░  [Integration tests]
Priorytet 3 (Przyszły):  15% ██░░░░░░░░  [E2E tests]
```

## 🤝 Contributing

Przy dodawaniu nowych testów:

1. Umieść testy w odpowiednim pliku w `tests/unit/`
2. Dodaj opisowe nazwy testów
3. Upewnij się, że wszystkie testy przechodzą: `npm test`
4. Sprawdź pokrycie: `npm run test:coverage`
5. Dokumentuj edge cases w komentarzach

## 📚 Dokumentacja

- [Jest Documentation](https://jestjs.io/docs/getting-started)
- [Chrome Extension API](https://developer.chrome.com/docs/extensions/reference/)
- [JSDOM](https://github.com/jsdom/jsdom)

## 🔗 Linki

- [Raport Analizy Słabych Punktów](../ANALYSIS.md)
- [Workflow Manager Source](../src/sidebar/workflow-manager.js)
- [CI/CD Configuration](../.github/workflows/test.yml) *(do dodania)*

---

**Ostatnia aktualizacja:** 2026-01-23
**Wersja test suite:** 1.0.0
**Status:** ✅ Wszystkie testy Priorytetu 1 ukończone
