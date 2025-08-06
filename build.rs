// Script de build para compilar arquivos Slint UI
fn main() {
    // Compila arquivo principal da interface Slint
    slint_build::compile("ui/app.slint").unwrap();
}
