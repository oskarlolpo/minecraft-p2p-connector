use anyhow::{Context, Result};
use igd_next::{search_gateway, Gateway, PortMappingProtocol, SearchOptions};
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use tokio::task;
use tracing::{error, info};

/// Хранит состояние проброшенного порта. 
/// При выходе из области видимости (Drop) порт будет автоматически закрыт на роутере.
pub struct UpnpMapping {
    gateway: Gateway,
    external_port: u16,
    protocol: PortMappingProtocol,
}

impl UpnpMapping {
    /// Пытается найти UPnP-шлюз и пробросить указанный UDP-порт.
    pub async fn attempt_map(local_port: u16, description: &str) -> Result<Self> {
        // 1. Автоматическое определение локального IP-адреса
        let local_ip = Self::get_local_ip()
            .context("Не удалось определить локальный IPv4 адрес хоста")?;
        
        let local_addr = SocketAddrV4::new(local_ip, local_port);
        info!("Локальный адрес определен как: {}", local_addr);

        // 2. Поиск шлюза в отдельном потоке (блокирующая операция)
        info!("Начинаем поиск UPnP-шлюза...");
        let gateway = task::spawn_blocking(|| {
            search_gateway(SearchOptions::default())
        })
        .await
        .context("Ошибка пула потоков (spawn_blocking) при поиске шлюза")?
        .context("UPnP-шлюз не найден в локальной сети")?;

        info!("UPnP-шлюз найден: {}", gateway);

        // 3. Подготовка параметров для проброса порта
        let protocol = PortMappingProtocol::UDP;
        let external_port = local_port; // Пытаемся занять такой же порт на роутере
        let lease_duration = 0; // 0 означает бессрочную аренду (до ручного удаления)
        let desc_owned = description.to_string();
        
        // Клонируем шлюз для передачи в замыкание (он содержит только данные для подключения)
        let gw_clone = gateway.clone();

        // 4. Добавление порта на роутер (блокирующая операция)
        task::spawn_blocking(move || {
            gw_clone.add_port(
                protocol,
                external_port,
                std::net::SocketAddr::V4(local_addr),
                lease_duration,
                &desc_owned,
            )
        })
        .await
        .context("Ошибка пула потоков (spawn_blocking) при добавлении порта")?
        .context("Роутер отказал в пробросе порта")?;

        info!(
            "UPnP: Порт {} (UDP) успешно проброшен на внутренний адрес {}",
            external_port, local_addr
        );

        Ok(Self {
            gateway,
            external_port,
            protocol,
        })
    }

    /// Вспомогательный метод для получения локального IPv4 через фиктивное UDP-подключение
    fn get_local_ip() -> Result<Ipv4Addr> {
        // Привязываемся к любому свободному локальному порту
        let socket = UdpSocket::bind("0.0.0.0:0")
            .context("Не удалось создать UDP сокет для проверки IP")?;
        
        // "Подключаемся" к публичному DNS Google. 
        // Реального сетевого пакета не отправляется, но ОС вычисляет маршрут и нужный интерфейс.
        socket.connect("8.8.8.8:80")
            .context("Не удалось проложить маршрут до публичного IP")?;
            
        let local_addr = socket.local_addr()
            .context("Не удалось получить локальный адрес привязанного сокета")?;

        match local_addr.ip() {
            std::net::IpAddr::V4(ipv4) => Ok(ipv4),
            _ => anyhow::bail!("Полученный локальный IP не является IPv4"),
        }
    }
}

// Реализация автоматической очистки порта при завершении работы приложения/удалении объекта
impl Drop for UpnpMapping {
    fn drop(&mut self) {
        info!(
            "UPnP: Запущена очистка сессии. Попытка удаления порта {}...",
            self.external_port
        );
        
        // Вызов remove_port блокирующий, но в рамках Drop его безопасно выполнять напрямую,
        // так как он должен гарантированно завершиться при выходе.
        match self.gateway.remove_port(self.protocol, self.external_port) {
            Ok(_) => info!(
                "UPnP: Порт {} (UDP) успешно удален с роутера.",
                self.external_port
            ),
            Err(e) => error!(
                "UPnP: Ошибка при удалении порта {} с роутера: {}",
                self.external_port, e
            ),
        }
    }
}
