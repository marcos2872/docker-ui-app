use std::env;
extern crate teste_docker;
use teste_docker::ssh::{SshClient, SshConnection};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configurações do servidor SSH (usando variáveis de ambiente para segurança)
    let host = "192.168.1.3".to_string();
    let username = "bot".to_string();
    let password = "bot".to_string();
    let private_key = env::var("SSH_PRIVATE_KEY").ok();

    println!("🔧 Testando serviço SSH...");
    println!("📡 Host: {}", host);
    println!("👤 Usuário: {}", username);

    // Cria conexão SSH
    let connection = SshConnection {
        host: host.clone(),
        port: 22,
        username: username.clone(),
        password,
        private_key,
        passphrase: None,
    };

    // Testa conexão
    let mut client = SshClient::new();

    println!("\n🔄 Conectando ao servidor SSH...");
    match client.connect(connection).await {
        Ok(_) => println!("✅ Conectado com sucesso!"),
        Err(e) => {
            println!("❌ Erro na conexão: {}", e);
            println!("\n💡 Dicas para resolver:");
            println!("   - Verifique se o SSH está rodando no servidor");
            println!("   - Confirme as credenciais");
            println!("   - Configure as variáveis de ambiente:");
            println!("     export SSH_HOST=192.168.1.100");
            println!("     export SSH_USER=seu_usuario");
            println!("     export SSH_PASSWORD=sua_senha");
            println!("     # OU");
            println!("     export SSH_PRIVATE_KEY=/caminho/para/chave_privada");
            return Err(e.into());
        }
    }

    // Teste 1: Comando simples
    println!("\n📋 Teste 1: Executando comando 'whoami'");
    match client.execute_command("whoami").await {
        Ok(result) => {
            println!("✅ Comando executado:");
            println!("   Saída: {}", result.stdout.trim());
            println!("   Código de saída: {}", result.exit_code);
        }
        Err(e) => println!("❌ Erro: {}", e),
    }

    // Teste 2: Informações do sistema
    println!("\n🖥️  Teste 2: Coletando informações do servidor");
    match client.get_server_info().await {
        Ok(info) => {
            println!("✅ Informações coletadas:");
            println!("   Hostname: {}", info.hostname);
            println!("   Uptime: {}", info.uptime);
            println!("   Uso de CPU: {}", info.cpu_usage);
            println!(
                "   Memória: {}",
                info.memory_usage.lines().next().unwrap_or("N/A")
            );
            println!(
                "   Disco: {}",
                info.disk_usage.lines().nth(1).unwrap_or("N/A")
            );
        }
        Err(e) => println!("❌ Erro: {}", e),
    }

    // Teste 3: Listar diretório
    println!("\n📁 Teste 3: Listando diretório home");
    match client.execute_command("ls -la ~").await {
        Ok(result) => {
            println!("✅ Conteúdo do diretório:");
            for line in result.stdout.lines().take(10) {
                println!("   {}", line);
            }
            if result.stdout.lines().count() > 10 {
                println!(
                    "   ... (mais {} linhas)",
                    result.stdout.lines().count() - 10
                );
            }
        }
        Err(e) => println!("❌ Erro: {}", e),
    }

    // Teste 4: Teste de upload/download (opcional)
    println!("\n📤 Teste 4: Testando upload de arquivo");

    // Cria arquivo temporário para teste
    let test_content = "Teste de upload SSH - teste-docker";
    std::fs::write("/tmp/ssh_test.txt", test_content)?;

    match client
        .upload_file("/tmp/ssh_test.txt", "/tmp/ssh_test_remote.txt")
        .await
    {
        Ok(_) => {
            println!("✅ Arquivo enviado com sucesso");

            // Verifica se o arquivo foi enviado
            match client.execute_command("cat /tmp/ssh_test_remote.txt").await {
                Ok(result) => {
                    if result.stdout.trim() == test_content {
                        println!("✅ Conteúdo verificado no servidor");
                    } else {
                        println!("⚠️  Conteúdo divergente no servidor");
                    }
                }
                Err(e) => println!("❌ Erro ao verificar arquivo: {}", e),
            }

            // Teste de download
            println!("\n📥 Teste 5: Testando download de arquivo");
            match client
                .download_file("/tmp/ssh_test_remote.txt", "/tmp/ssh_test_download.txt")
                .await
            {
                Ok(_) => match std::fs::read_to_string("/tmp/ssh_test_download.txt") {
                    Ok(content) => {
                        if content.trim() == test_content {
                            println!("✅ Download verificado com sucesso");
                        } else {
                            println!("⚠️  Conteúdo do download divergente");
                        }
                    }
                    Err(e) => println!("❌ Erro ao ler arquivo baixado: {}", e),
                },
                Err(e) => println!("❌ Erro no download: {}", e),
            }

            // Limpa arquivos de teste
            let _ = client
                .execute_command("rm -f /tmp/ssh_test_remote.txt")
                .await;
        }
        Err(e) => println!("❌ Erro no upload: {}", e),
    }

    // Limpa arquivo local
    let _ = std::fs::remove_file("/tmp/ssh_test.txt");
    let _ = std::fs::remove_file("/tmp/ssh_test_download.txt");

    println!("\n🔚 Desconectando...");
    client.disconnect();
    println!("✅ Testes concluídos!");

    Ok(())
}
