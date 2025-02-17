# Плагины к Realtime серверу для реализации игровой логики на сервере

## Возможности

- отдельные приложения на любом языке программирования (поддержка gRPC + подключение so|dll);
- получают все команды битвы через UDP протокол как обычный клиент;
- используют gRPC сервис realtime сервера для управления и конфигурирования, например могут выкинуть игрока из битвы,
  настроить права доступа и так далее;

## Плагин на основе Unity Dedicated Server (UDS)

Так как UDS может обслуживать только одну комнату - то для каждой комнаты запускается отдельный экземпляр UDS. За это
отвечает plugin-runner, который запускает и останавливает UDS в зависимости от состава комнаты. Сервер получает ключ
игрока для входа в realtime сервер, а также адрес для входа и адрес на gRPC интерфейс для управления реалтайм сервером.

Далее он осуществляет прослушивание и перехват команд, создает новые сетевые команды, определяет жизненный цикл битвы.
