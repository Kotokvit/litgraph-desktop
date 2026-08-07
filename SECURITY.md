## ⚠️ ВАЖНОЕ ПРЕДУПРЕЖДЕНИЕ О БЕЗОПАСНОСТИ

GitHub токен — это полный доступ к твоему аккаунту. Никогда не публикуй его:
- в чатах с AI
- в коммитах
- в issues/pull requests
- в скриншотах

## Что делать если токен утёк

1. НЕМЕДЛЕННО зайди на https://github.com/settings/tokens
2. Найди утёкший токен
3. Нажми **Delete** (отозвать)
4. Создай новый токен с теми же правами
5. Сохрани его в безопасном месте (например в password manager)

## Как безопасно работать с токеном локально

### Вариант 1: переменная окружения (рекомендуется)

```bash
# В ~/.bashrc или ~/.zshrc
export GITHUB_TOKEN="ghp_..."

# Использовать в скриптах
gh repo create litgraph-desktop --public --source=. --push
```

### Вариант 2: GitHub CLI

```bash
# Установить gh
sudo apt install gh

# Авторизоваться (токен сохранится локально в ~/.config/gh/hosts.yml)
gh auth login

# Создать репозиторий
gh repo create litgraph-desktop --public --source=. --push
```

### Вариант 3: git credentials

```bash
# Сохранить токен в git credential helper
git config --global credential.helper store
echo "https://Kotokvit:ghp_...@github.com" > ~/.git-credentials
chmod 600 ~/.git-credentials
```

## Права токена для этого проекта

Минимально необходимые:
- `repo` — полный доступ к репозиториям (создание, push, etc.)
- `workflow` — для GitHub Actions

НЕ давай:
- `delete_repo`
- `admin:org`
- `admin:public_key`
- `user` (если не нужно)
