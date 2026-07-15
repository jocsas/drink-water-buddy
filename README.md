# Hydrate Buddy App

Aplicativo Electron independente para lembretes de hidratação, com personagem
na tela, ações de confirmação/snooze e ícone na barra do sistema.

Esta pasta não contém o repositório original, `node_modules`, builds ou assets
de desenvolvimento. Ela mantém apenas o necessário para executar e empacotar
o app.

## Executar localmente

```bash
npm install
npm start
```

## Gerar o app para macOS

```bash
npm run dist:mac
```

Os arquivos `.dmg` e `.zip` serão gerados em `dist/`. O build usa a arquitetura
do Mac em que o comando for executado.

Para Windows:

```bash
npm run dist:win
```

## Estrutura

- `main.js`: janela, tray, agenda e armazenamento local.
- `preload.js`: ponte segura entre Electron e a interface.
- `renderer/`: personagem, balão e interações.
- `assets/`: imagens usadas pelo app.
- `build/icon.png`: ícone do aplicativo.

O projeto mantém a licença MIT da base reaproveitada. Antes de publicar, troque
o nome, `appId`, ícone e identidade visual pelos dados do seu projeto.
