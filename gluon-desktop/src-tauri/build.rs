fn main() {
    // Powiedz Cargo, aby ponownie uruchomił ten skrypt budowania, jeśli coś zmieni się w folderze migracji.
    // To jest kluczowe, aby `sqlx::migrate!()` zawsze miało najnowsze migracje.
    println!("cargo:rerun-if-changed=migrations");
    
    // Upewniamy się, że środowisko budowania jest świadome zmian w zależnościach C/C++
    // Jest to przydatne przy pracy z tree-sitter, chociaż większość crate'ów radzi sobie sama.
    // Na Windows (MSVC) pomaga to w debugowaniu problemów z linkowaniem.
    println!("cargo:rerun-if-env-changed=CC");
    println!("cargo:rerun-if-env-changed=CXX");

    tauri_build::build()
}