extern crate teste_docker;
use teste_docker::docker::{DockerManager, DockerManagement};
use teste_docker::ssh::SshConnection;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configuração do servidor remoto
    let host = "192.168.1.3".to_string();
    let username = "bot".to_string();
    let password = "bot".to_string();

    println!("🐳 Testando Docker Remoto via SSH");
    println!("📡 Servidor: {}@{}", username, host);
    println!("{}", "═".repeat(60));

    // Configurar conexão SSH
    let ssh_connection = SshConnection {
        host: host.clone(),
        port: 22,
        username: username.clone(),
        password,
        private_key: None,
        passphrase: None,
    };

    // Conectar ao Docker remoto
    let mut docker = DockerManager::new();

    println!("\n🔄 Conectando ao servidor remoto...");
    match docker.connect(ssh_connection).await {
        Ok(_) => println!("✅ Conectado com sucesso!"),
        Err(e) => {
            println!("❌ Erro na conexão: {}", e);
            println!("\n💡 Dicas:");
            println!("   - Configure as variáveis de ambiente:");
            println!("     export REMOTE_HOST=192.168.1.100");
            println!("     export REMOTE_USER=seu_usuario");
            println!("     export REMOTE_PASSWORD=sua_senha");
            println!("   - Certifique-se que Docker está instalado no servidor remoto");
            return Err(e);
        }
    }

    // Teste 1: Informações do Docker
    println!("\n📊 Teste 1: Informações do Docker");
    match docker.get_docker_info().await {
        Ok(info) => {
            println!("✅ Docker Info:");
            println!("   Versão: {}", info.version);
            println!("   Containers rodando: {}", info.containers_running);
            println!("   Containers parados: {}", info.containers_stopped);
            println!("   Containers pausados: {}", info.containers_paused);
            println!("   Imagens: {}", info.images);
        }
        Err(e) => println!("❌ Erro: {}", e),
    }

    // Teste 2: Listar containers
    println!("\n📦 Teste 2: Listando containers");
    match docker.list_containers().await {
        Ok(containers) => {
            println!("✅ Encontrados {} containers:", containers.len());
            for container in containers.iter().take(5) {
                println!("   📦 {} ({})", container.name, container.state);
                println!("      Image: {}", container.image);
                println!("      Status: {}", container.status);
                if !container.ports.is_empty() {
                    println!("      Portas: {:?}", container.ports);
                }
                println!();
            }

            if containers.len() > 5 {
                println!("   ... e mais {} containers", containers.len() - 5);
            }
        }
        Err(e) => println!("❌ Erro: {}", e),
    }

    // Teste 3: Listar imagens
    println!("\n🖼️  Teste 3: Listando imagens");
    match docker.list_images().await {
        Ok(images) => {
            println!("✅ Encontradas {} imagens:", images.len());
            for image in images.iter().take(5) {
                let size_mb = image.size as f64 / 1_048_576.0;
                println!(
                    "   🖼️  {} ({:.1} MB)",
                    image.tags.first().unwrap_or(&"<none>".to_string()),
                    size_mb
                );
            }

            if images.len() > 5 {
                println!("   ... e mais {} imagens", images.len() - 5);
            }
        }
        Err(e) => println!("❌ Erro: {}", e),
    }

    // Teste 4: Listar networks
    println!("\n🌐 Teste 4: Listando redes");
    match docker.list_networks().await {
        Ok(networks) => {
            println!("✅ Encontradas {} redes:", networks.len());
            for network in &networks {
                println!("   🌐 {} ({})", network.name, network.driver);
            }
        }
        Err(e) => println!("❌ Erro: {}", e),
    }

    // Teste 5: Listar volumes
    println!("\n💾 Teste 5: Listando volumes");
    match docker.list_volumes().await {
        Ok(volumes) => {
            println!("✅ Encontrados {} volumes:", volumes.len());
            for volume in volumes.iter().take(5) {
                println!("   💾 {} ({})", volume.name, volume.driver);
            }
        }
        Err(e) => println!("❌ Erro: {}", e),
    }

    // Teste 6: Gerenciamento de containers (se houver algum)
    println!("\n⚙️  Teste 6: Teste de gerenciamento");
    match docker.list_containers().await {
        Ok(containers) => {
            if let Some(container) = containers.first() {
                println!("Testando com container: {}", container.name);

                // Teste logs
                match docker.get_container_logs(&container.name, Some(10)).await {
                    Ok(logs) => {
                        if !logs.is_empty() {
                            println!("✅ Últimas 10 linhas dos logs:");
                            for line in logs.lines().take(3) {
                                println!("   📝 {}", line);
                            }
                            if logs.lines().count() > 3 {
                                println!("   ... (mais {} linhas)", logs.lines().count() - 3);
                            }
                        } else {
                            println!("ℹ️  Container não possui logs");
                        }
                    }
                    Err(e) => println!("❌ Erro ao obter logs: {}", e),
                }

                // Se o container estiver parado, tente iniciar
                if container.state == "exited" {
                    println!("\n🔄 Container parado, tentando iniciar...");
                    match docker.start_container(&container.name).await {
                        Ok(_) => println!("✅ Container iniciado com sucesso!"),
                        Err(e) => println!("❌ Erro ao iniciar container: {}", e),
                    }
                } else if container.state == "running" {
                    println!("\nℹ️  Container já está rodando");
                }
            } else {
                println!("ℹ️  Nenhum container encontrado para teste");
            }
        }
        Err(e) => println!("❌ Erro: {}", e),
    }

    println!("\n🔚 Desconectando...");
    docker.disconnect();
    println!("✅ Testes de Docker remoto concluídos!");

    Ok(())
}
