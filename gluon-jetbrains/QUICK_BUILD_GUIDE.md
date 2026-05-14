# 🚀 Szybki przewodnik budowania (bez gradlew.bat)

## Problem
Brak plików `gradlew.bat` - są potrzebne dodatkowe pliki wrapper które PyCharm nie utworzył automatycznie.

## ✅ Rozwiązanie 1: Użyj PyCharm GUI (NAJŁATWIEJSZE)

### Krok 1: Otwórz panel Gradle w PyCharm

1. **View → Tool Windows → Gradle**
   - Lub kliknij ikonę słonia 🐘 po prawej stronie okna

2. Jeśli nie widzisz panelu Gradle:
   - Kliknij prawym na `build.gradle.kts` w drzewie projektu
   - Wybierz **"Import Gradle Project"** lub **"Link Gradle Project"**

### Krok 2: Uruchom buildPlugin

W panelu Gradle (po prawej):

```
🐘 gluon-jetbrains
  └── 📁 Tasks
      └── 📁 intellij
          └── ⚡ buildPlugin    ← KLIKNIJ DWUKROTNIE
```

### Krok 3: Poczekaj na zakończenie

W dolnym oknie zobaczysz:
```
> Task :compileKotlin
> Task :compileJava
> Task :jar
> Task :buildPlugin

BUILD SUCCESSFUL in 2m 15s
```

### Krok 4: Znajdź wtyczkę

Plik ZIP będzie w:
```
C:\Users\PC\Desktop\Gluon-v2\gluon-jetbrains\build\distributions\Gluon-1.0-SNAPSHOT.zip
```

---

## ✅ Rozwiązanie 2: Użyj menu Run w PyCharm

1. **Kliknij prawym** na plik `build.gradle.kts`
2. Wybierz **"Run 'gluon-jetbrains [buildPlugin]'"**
3. PyCharm uruchomi zadanie automatycznie

---

## ✅ Rozwiązanie 3: Stwórz gradlew ręcznie (zaawansowane)

Jeśli koniecznie chcesz używać terminala:

### Opcja A: Pobierz wrapper z internetu

```powershell
# 1. Stwórz folder gradlew
New-Item -ItemType Directory -Force -Path "gradle\wrapper"

# 2. Pobierz gradle-wrapper.jar (wersja 8.5)
$url = "https://raw.githubusercontent.com/gradle/gradle/v8.5.0/gradle/wrapper/gradle-wrapper.jar"
Invoke-WebRequest -Uri $url -OutFile "gradle\wrapper\gradle-wrapper.jar"

# 3. Pobierz gradlew.bat
$url = "https://raw.githubusercontent.com/gradle/gradle/v8.5.0/gradlew.bat"
Invoke-WebRequest -Uri $url -OutFile "gradlew.bat"

# 4. Pobierz gradlew (Unix)
$url = "https://raw.githubusercontent.com/gradle/gradle/v8.5.0/gradlew"
Invoke-WebRequest -Uri $url -OutFile "gradlew"

# 5. Teraz możesz użyć
.\gradlew.bat buildPlugin
```

### Opcja B: Zainstaluj Gradle globalnie

```powershell
# Przez Chocolatey (wymaga uprawnień admina)
choco install gradle

# Po instalacji
cd C:\Users\PC\Desktop\Gluon-v2\gluon-jetbrains
gradle wrapper
.\gradlew.bat buildPlugin
```

---

## 🎯 ZALECANA METODA

**Użyj PyCharm GUI (Rozwiązanie 1)**

To jest najszybsze i najpewniejsze rozwiązanie. PyCharm ma wbudowanego Gradle i nie potrzebujesz plików wrapper.

### Dlaczego PyCharm GUI?
- ✅ Nie wymaga gradlew.bat
- ✅ Nie wymaga instalacji Gradle
- ✅ Automatyczna konfiguracja
- ✅ Wizualna informacja o postępie
- ✅ Łatwe debugowanie błędów

---

## 📦 Co zrobić po zbudowaniu

1. **Znajdź plik:**
   ```
   build\distributions\Gluon-1.0-SNAPSHOT.zip
   ```

2. **Zainstaluj w PyCharm (lub innym JetBrains IDE):**
   - File → Settings → Plugins
   - ⚙️ → Install Plugin from Disk...
   - Wybierz `Gluon-1.0-SNAPSHOT.zip`
   - Restart IDE

3. **Uruchom Gluon Desktop App** (port 8743)

4. **Otwórz projekt** - powinieneś zobaczyć:
   ```
   🔔 Gluon Connected
   Successfully connected to Gluon Desktop App
   ```

---

## 🐛 Rozwiązywanie problemów

### Panel Gradle nie pojawia się w PyCharm

**Sprawdź czy plugin jest włączony:**
1. File → Settings → Plugins
2. Szukaj: "Gradle"
3. Upewnij się że jest zaznaczony ✅
4. Restart PyCharm

### "Cannot find Gradle tasks"

**Reload projektu Gradle:**
1. W panelu Gradle kliknij ikonę odświeżania 🔄
2. Lub: Kliknij prawym na projekt → Reload Gradle Project

### Build się nie udaje

**Sprawdź logi:**
1. W dolnym oknie PyCharm kliknij zakładkę "Build"
2. Szukaj czerwonych błędów
3. Najczęstsze problemy:
   - Brak internetu (nie może pobrać zależności)
   - Niewłaściwa wersja JDK (potrzebna 17+)
   - Zablokowany port przez firewall

---

## ✅ Podsumowanie - 3 proste kroki

1. **Otwórz projekt w PyCharm**
2. **Gradle panel → intellij → buildPlugin** (dwukrotnie kliknij)
3. **Znajdź ZIP w:** `build\distributions\`

**Gotowe!** 🎉
