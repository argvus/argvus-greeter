---
title: Greeter
description: Configure a interface greetd do ARGVUS.
slug: pt/0.4.0/docs/user-guide/sessions/greeter
---

`argvus-greeter` é o greeter TUI orientado ao teclado usado pelo serviço greetd. A autenticação é feita pelo greetd através do PAM; o greeter apresenta a seleção de conta e sessão, mas não implementa a autenticação por senha.

O helper de configuração é `argvus-greeter-setup`. Em um sistema instalado, use:

```sh
sudo argvus-greeter-setup --enable
```

Use `sudo argvus-greeter-setup --now` quando o greetd deve ser reiniciado imediatamente. O helper preserva um backup de `/etc/greetd/config.toml` antes de instalar a configuração do greetd do ARGVUS. Use `argvus-greeter-setup --help` para consultar as opções instaladas.

O greeter lê dados locais de `argvus-accounts`, autentica pelo greetd e entrega o contexto do usuário à sessão. Os avatares são procurados no arquivo `.face` do usuário, no local de ícones do AccountsService ou na entrada `Icon=` declarada. `argvus-accounts` pode criar uma imagem `.face` validada; o greeter não edita avatares.

A configuração do greeter é `/etc/argvus/greeter.toml`. Para atualizar as projeções públicas dos temas usadas antes da autenticação, sem alterar a configuração do greetd, execute:

```sh
sudo argvus-greeter-setup --sync-themes
```

Antes da autenticação, o greeter não lê o diretório privado do usuário selecionado. A seleção de tema pela aparência do ARGVUS publica o tema validado em `/var/lib/argvus/greeter/themes/<uid>` como uma projeção atômica pertencente ao UID. `argvus-greeter-setup --sync-themes` continua sendo o comando administrativo de ressincronização. A TUI de login e a transição após o login usam essa projeção, incluindo o acento opcional em `<uid>.accent`; `.active-theme` e `.accent-color` continuam como fallbacks de compatibilidade. Se o tema selecionado não estiver disponível, o greeter usa o fallback determinístico `argvus-dark` empacotado em vez de falhar.

O `/etc/argvus/greeter.toml` administrativo controla separadamente a
superfície anterior ao login. Em `[appearance]`, use
`transparency_enabled`, `transparency_value`, `blur_enabled` e `blur_value`:

```toml
[appearance]
transparency_enabled = true
transparency_value = 50
blur_enabled = true
blur_value = 50
```

Os percentuais são limitados a `0..=100`. O loader Rust tipado passa os
valores normalizados ao Kitty e ao Hyprland mínimo. O
`~/.config/argvus/config.json` do usuário não é lido antes da autenticação. O
blur só é visualmente perceptível quando existe conteúdo atrás da superfície
Kitty.

Greeter, overlay de carregamento da sessão e splash de boot são componentes separados.
