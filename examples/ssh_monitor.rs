use std::env;
use std::time::Duration;
// Para examples dentro do próprio crate, use paths relativos ou crate::
extern crate teste_docker;
use teste_docker::ssh::{SshClient, SshConnection};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = "192.168.1.3".to_string();
    let username = "bot".to_string();
    let password = "bot".to_string();
    let private_key = env::var("SSH_PRIVATE_KEY").ok();

    println!("🖥️  Monitor SSH - Conectando a {}@{}", username, host);

    let connection = SshConnection {
        host: host.clone(),
        port: 22,
        username: username.clone(),
        password: password,
        private_key,
        passphrase: None,
    };

    let mut client = SshClient::new();

    match client.connect(connection).await {
        Ok(_) => println!("✅ Conectado!"),
        Err(e) => {
            println!("❌ Erro: {}", e);
            return Err(e.into());
        }
    }

    println!("\n📊 Iniciando monitoramento (Ctrl+C para parar)...\n");

    loop {
        // Limpa tela (funciona na maioria dos terminais)
        print!("\x1B[2J\x1B[1;1H");

        println!("🖥️  Monitor SSH - {}", host);
        println!("{}", "═".repeat(60));

        // Informações do sistema
        match client.get_server_info().await {
            Ok(info) => {
                println!("🏷️  Hostname: {}", info.hostname);
                println!("⏱️  Uptime: {}", info.uptime);
                println!("🧠 CPU: {}", info.cpu_usage);

                // Exibe apenas a primeira linha do free (header) e segunda (dados)
                let memory_lines: Vec<&str> = info.memory_usage.lines().collect();
                if memory_lines.len() >= 2 {
                    println!("💾 {}", memory_lines[0]); // Header
                    println!("   {}", memory_lines[1]); // Mem data
                }

                // Exibe header e linha do root filesystem
                let disk_lines: Vec<&str> = info.disk_usage.lines().collect();
                if disk_lines.len() >= 2 {
                    println!("💿 {}", disk_lines[0]); // Header
                    println!("   {}", disk_lines[1]); // Root filesystem
                }
            }
            Err(e) => println!("❌ Erro ao coletar informações: {}", e),
        }

        println!("\n📈 Processos mais pesados:");
        match client
            .execute_command("ps aux --sort=-%cpu | head -6")
            .await
        {
            Ok(result) => {
                for line in result.stdout.lines() {
                    println!("   {}", line);
                }
            }
            Err(e) => println!("❌ Erro: {}", e),
        }

        println!("\n🌐 Conexões de rede:");
        match client.execute_command("ss -tuln | head -10").await {
            Ok(result) => {
                for line in result.stdout.lines() {
                    println!("   {}", line);
                }
            }
            Err(e) => println!("❌ Erro: {}", e),
        }

        println!("\n⏰ Próxima atualização em 10 segundos...");
        sleep(Duration::from_secs(10)).await;
    }
}
