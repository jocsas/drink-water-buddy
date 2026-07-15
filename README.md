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

Build local de teste, sem assinatura de distribuição:

```bash
npm run dist:mac:dev
```

Build para compartilhar diretamente com amigos, sem notarização Apple:

```bash
npm run dist:mac:friends
```

Esse comando gera o build `arm64`, que e o correto para Macs Apple Silicon
incluindo M1, M2, M3 e M4. O arquivo para mandar e:

```text
dist/HydrateBuddy-0.1.0-arm64.dmg
```

Ele tambem verifica que o app ficou como `LSUIElement=true`, ou seja, fora do
Dock. Como ele nao e notarizado, o macOS pode mostrar um aviso no primeiro uso.
Quem receber deve montar o `.dmg`, arrastar o app para Applications e abrir pelo
Finder com Control-click ou botao direito no app, depois escolher "Open". Use
isso apenas com pessoas que confiam em voce e sabem que e um build direto.

Se voce quiser mandar um unico arquivo para Macs Intel e Apple Silicon:

```bash
npm run dist:mac:friends:universal
```

Release para distribuir:

```bash
npm run dist:mac
```

O build de release exige um certificado `Developer ID Application` válido e
credenciais de notarização da Apple. Configure uma das opções abaixo antes de
rodar:

```bash
export APPLE_API_KEY=/caminho/AuthKey_XXXXXXXXXX.p8
export APPLE_API_KEY_ID=XXXXXXXXXX
export APPLE_API_ISSUER=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
```

ou:

```bash
export APPLE_ID=voce@exemplo.com
export APPLE_APP_SPECIFIC_PASSWORD=xxxx-xxxx-xxxx-xxxx
export APPLE_TEAM_ID=XXXXXXXXXX
```

Os arquivos `.dmg`, `.zip`, `.blockmap` e `latest-mac.yml` serão gerados em
`dist/`. O app macOS e os helpers sao configurados com `LSUIElement=true`, para
ficarem fora do Dock e aparecerem apenas na barra do topo.

Para criar um rascunho de release no GitHub:

```bash
export GH_TOKEN=ghp_seu_token
npm run release:github
```

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
