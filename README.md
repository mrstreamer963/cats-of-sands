# Cats of Sands

Браузерная демосцена на Vite и JavaScript. Rust/WASM обновляет мир через `bevy_ecs`, а Canvas 2D отображает его состояние.

## Запуск

Понадобятся Node.js, совместимый с Vite 8 (20.19+ или 22.12+), Rust 1.95+, `wasm-pack` и `make`.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --locked
npm ci
make dev
```

Открой адрес, указанный Vite. Нажми «Запустить» для базовой сцены или включи «Пустынный спутник», чтобы ускорить первый объект и добавить второй. Изменения галочек сохраняются в браузере и применяются при следующем нажатии «Запустить».

## Сборка и проверка

```sh
cargo test --locked
npm run build
npm run preview
```

Если установлен `cargo-outdated`, проверку версий зависимостей можно запустить из корня командой `cargo outdated`.

`dev` и `build` собирают WASM перед запуском Vite. После изменения Rust-кода перезапусти dev-сервер или выполни `npm run wasm`.

## Конфигурация

Базовая сцена находится в [base.yaml](public/config/base.yaml). Поставляемые аддоны лежат в [public/config/addons](public/config/addons), а порядок применения задан в [каталоге](src/addons.js). Ключи в `entities` — стабильные ID. Отображения объединяются рекурсивно, списки заменяются целиком, а значение более позднего слоя имеет приоритет.
