Gluon v2 - Pliki Instalacyjne
Witamy w oficjalnym repozytorium dystrybucyjnym Gluon v2. Znajdziesz tutaj pliki niezbędne do zainstalowania aplikacji desktopowej (silnika) oraz rozszerzenia przeglądarki (interfejsu).

Gluon v2 to precyzyjny menedżer kontekstu AI dla programistów, który łączy Twoje lokalne projekty z interfejsami webowymi Claude, Gemini i AI Studio .



Ważne: Do pełnego działania wymagane są oba komponenty oraz płatny klucz licencyjny dostępny na stronie https://ai-gluon.com/



🗂️ Pliki w tym Repozytorium
Gluon-v2-[wersja]_x64_en-US.msi

Co to jest: Aplikacja desktopowa (silnik) dla systemu Windows.


Rola: Działa w tle, zarządzając projektami, bazą danych i komunikacją . Zbudowana w Tauri + Rust, zużywa minimalne zasoby (<40MB RAM).



gluon-v2-extension_[wersja].zip

Co to jest: Rozszerzenie dla przeglądarki Chrome (interfejs).


Rola: Panel boczny (sidebar) działający na stronach AI (Claude, Gemini, AI Studio), który pozwala na interakcję z plikami .


⚙️ Wymagania Systemowe

System Operacyjny: Windows 


Przeglądarka: Google Chrome (lub inna oparta na Chromium) 


Licencja: Płatny klucz licencyjny (do nabycia na Gumroad).



🚀 Instrukcja Instalacji (Krok po Kroku)
Krok 1: Zdobądź Klucz Licencyjny Zanim zaczniesz, musisz posiadać klucz licencyjny.

Klucz licencyjny jest dostępny na store.ai-gluon.com (Gumroad).


Jest to jednorazowa opłata za dożywotnią licencję.



Krok 2: Zainstaluj Silnik Desktopowy (MSI)

Pobierz najnowszy plik Gluon-v2-[wersja]_x64_en-US.msi z sekcji "Releases" tego repozytorium.

Uruchom instalator i postępuj zgodnie z instrukcjami.

Po instalacji uruchom aplikację Gluon v2. Powinna pojawić się w zasobniku systemowym (system tray).

Otwórz ustawienia aplikacji (klikając ikonę w zasobniku) i wklej swój klucz licencyjny, aby ją aktywować.

Krok 3: Zainstaluj Rozszerzenie Chrome (ZIP)

Pobierz najnowszy plik gluon-v2-extension_[wersja].zip z sekcji "Releases".

Rozpakuj plik .zip do folderu w bezpiecznej lokalizacji (np. C:\Program Files\GluonExtension).

Otwórz przeglądarkę Chrome i przejdź do chrome://extensions.

W prawym górnym rogu włącz "Tryb dewelopera" (Developer mode).

Kliknij przycisk "Załaduj rozpakowane" (Load unpacked).

Wybierz folder, do którego wcześniej rozpakowałeś pliki wtyczki (np. C:\Program Files\GluonExtension).

Wtyczka Gluon v2 powinna pojawić się na liście. Przypnij ją do paska narzędzi, aby mieć do niej łatwy dostęp.

Krok 4: Uruchomienie

Upewnij się, że aplikacja desktopowa Gluon v2 jest uruchomiona (widoczna w zasobniku systemowym).

Przejdź do claude.ai, gemini.google.com lub aistudio.google.com .

Kliknij ikonę Gluon na pasku narzędzi Chrome, aby otworzyć panel boczny.

Jeśli wszystko działa, wskaźnik połączenia w panelu bocznym powinien zaświecić się na zielono, a Ty powinieneś zobaczyć swoje projekty (dodane w aplikacji desktopowej).

💡 Czym jest Gluon?
Gluon nie jest kolejnym agentem AI. To narzędzie stworzone dla programistów, którzy cenią sobie kontrolę.





Zamiast pozwalać agentowi AI na samodzielne działanie (i generowanie masy niepotrzebnego kodu), Gluon działa jak "skalpel". To Ty decydujesz, które DOKŁADNIE pliki, fragmenty struktury projektu i prompty systemowe trafią do AI jako kontekst.

Kluczowe Zalety

Pełna Kontrola: Ty pozostajesz "w fotelu kierowcy". Koniec z "czarnymi skrzynkami" agentów.


Oszczędność Tokenów: Przekazując tylko minimalny, niezbędny kontekst, drastycznie obniżasz zużycie tokenów.


Praca na Darmowych Planach: Gluon jest zoptymalizowany do pracy z darmowymi interfejsami webowymi, dając Ci profesjonalny workflow bez kosztów API (np. przy użyciu AI Studio) .




Inteligentne Funkcje: Gluon posiada inteligentne nakładki UI (Auto-Select, Context Handoff), które automatyzują powtarzalne części pracy, jednocześnie pozostawiając Ci ostateczną decyzję .

Pełną dokumentację znajdziesz pod adresem https://ai-gluon.com/guide/


💬 Społeczność i Wsparcie
Masz pytania, sugestie lub znalazłeś błąd? Dołącz do naszej społeczności na Discordzie!


Link do Discorda: Dołącz do serwera Gluon v2 https://discord.gg/2wqpwpCq