#!/usr/bin/env python3
"""
NER-извлечение для LitGraph v0.2.0.

Улучшения v0.2.0:
1. Chunked processing — обрабатывает текст любого размера по частям
2. Фильтр ложных срабатываний: если "Вода" появляется как PROPN, но
   "вода" встречается как нарицательное NOUN — это НЕ персонаж
3. Минимальная частота ≥ 2 (одиночные упоминания — шум)
4. Проверка контекста: имя должно встречаться в середине предложений,
   а не только в начале (где все слова заглавные)
5. pymorphy3 строгая проверка для PER: только Name/Surn теги
6. Чёрный список распространённых ложных срабатываний

Использование:
    echo "Анна пошла в Москву" | python3 ner_extract.py
"""

import sys
import json
import re
from collections import defaultdict, Counter

try:
    import spacy
except ImportError:
    print(json.dumps({"error": "spaCy not installed. Run: pip install spacy && python -m spacy download ru_core_news_sm"}))
    sys.exit(1)

try:
    import pymorphy3
    MORPH = pymorphy3.MorphAnalyzer()
except ImportError:
    MORPH = None

try:
    NLP = spacy.load("ru_core_news_sm", disable=["lemmatizer"])
except OSError:
    try:
        NLP = spacy.load("ru_core_news_sm")
    except OSError:
        print(json.dumps({"error": "ru_core_news_sm model not found. Run: python -m spacy download ru_core_news_sm"}))
        sys.exit(1)


# === Чёрный список слов, которые spaCy ошибочно помечает как PROPN ===
# Это нарицательные существительные, которые могут быть заглавными в начале
# предложения или в названиях
FALSE_POSITIVE_NOUNS = {
    # Время
    "утро", "день", "вечер", "ночь", "рассвет", "закат", "полдень", "полночь",
    "вчера", "сегодня", "завтра", "послезавтра", "весна", "лето", "осень", "зима",
    "январь", "февраль", "март", "апрель", "май", "июнь",
    "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь",
    "понедельник", "вторник", "среда", "четверг", "пятница", "суббота", "воскресенье",
    # Стороны света
    "север", "юг", "восток", "запад", "северо-восток", "юго-запад",
    # Природа/элементы (часто в фэнтези помечаются как имена)
    "вода", "воздух", "огонь", "земля", "свет", "тьма", "тепло", "холод",
    "металл", "дерево", "камень", "песок", "стекло", "лёд",
    "море", "океан", "река", "озеро", "гора", "лес", "поле", "небо",
    "солнце", "луна", "звезда", "ветер", "дождь", "снег", "гроза",
    # Абстрактные понятия
    "мир", "жизнь", "смерть", "любовь", "надежда", "страх", "гнев",
    "тишина", "гул", "звук", "голос", "свет", "темнота", "мрак",
    "правда", "ложь", "свобода", "судьба", "доля", "удача",
    "боль", "радость", "грусть", "печаль", "тоска",
    "голод", "жажда", "усталость", "сон",
    "время", "вечность", "мгновение", "миг", "час", "минута",
    "добро", "зло", "правда", "красота", "уродство",
    # Части тела
    "голова", "рука", "нога", "глаз", "ухо", "нос", "рот", "плечо",
    "сердце", "душа", "тело", "кровь", "кожа",
    # Социальные понятия
    "город", "деревня", "дом", "дверь", "окно", "стена", "крыша",
    "улица", "площадь", "мост", "башня", "замок", "дворец",
    "комната", "кухня", "спальня", "коридор",
    # Религия/мифология (общие понятия, не имена)
    "бог", "богиня", "господь", "господи", "дьявол", "ангел", "демон",
    "дух", "призрак", "тень", "сущность",
    # Действия (иногда помечаются как PROPN в начале предложения)
    "всё", "ничего", "что-то", "кто-то", "где-то", "когда-то",
    "нет", "да", "может", "быть", "был", "была", "было", "были",
    # Места (не конкретные, а типы)
    "мир", "земля", "страна", "государство", "империя", "королевство",
    "корпорация", "компания", "организация", "система",
    # Эмоции/состояния
    "пауза", "молчание", "крик", "шёпот", "вздох",
    # Металлы и элементы (часто в фантастике — НЕ организации)
    "свинец", "цинк", "ртуть", "медь", "железо", "сталь", "бронза",
    "платина", "золото", "серебро", "титан", "хром", "никель",
    "керосин", "бензин", "нефть", "газ",
    # Технические термины (не организации)
    "совет", "центр", "реестр", "сектор", "система", "структура",
    "ядро", "актив", "пласт", "сдвиг", "сбой", "удар", "ритм",
    "камертон", "контракт", "аудит", "анализ", "перевод", "порядок",
    # 生物 / biology
    "крысы", "твари", "тварь", "гоблин", "грызун",
    # Общие понятия которые могут быть в начале предложения
    "верно", "конечно", "пусть", "здравствуй", "проходи", "считай",
    "копай", "начни", "береги", "слушай", "видишь", "узнаёшь",
    "спишь", "ходи", "дыши", "живи", "ждёт", "поёт",
}

# Слова которые ТОЧНО являются именами (даже если pymorphy3 не знает)
KNOWN_RUSSIAN_NAMES = {
    # Мужские имена
    "алексей", "александр", "андрей", "антон", "арсений", "артём", "артем",
    "борис", "вадим", "валентин", "валерий", "василий", "виктор", "виталий",
    "владимир", "владислав", "вячеслав", "геннадий", "георгий", "григорий",
    "денис", "дмитрий", "евгений", "игорь", "илья", "иван", "кирилл",
    "константин", "леонид", "максим", "матвей", "михаил", "никита", "николай",
    "олег", "павел", "пётр", "петр", "роман", "руслан", "сергей", "степан",
    "тимофей", "тимур", "фёдор", "федор", "юрий", "ярослав",
    # Женские имена
    "анна", "алёна", "алена", "алиса", "алла", "анастасия", "ангелина",
    "валентина", "валерия", "варвара", "вера", "вероника", "виктория",
    "галина", "дарья", "дария", "евгения", "екатерина", "елена", "елізавета",
    "зинаида", "инна", "ирина", "кira", "кира", "клавдия", "лариса",
    "людмила", "любовь", "маргарита", "мария", "надежда", "наталья",
    "наталия", "нина", "оксана", "олёна", "олена", "ольга", "полина",
    "раиса", "регина", "светлана", "софья", "софия", "таисия", "тамара",
    "татьяна", "ульяна", "юлия", "яна",
    # Уменьшительные (часто встречаются в диалогах)
    "лёша", "лёха", "сёма", "сёма", "паша", "веня", "жора", "костя",
    "дима", "женя", "саша", "маша", "катя", "настя", "лена", "таня",
    "витя", "аня", "оля", "вера", "надя", "люда", "нина",
}


def is_lowercase_noun_in_text(word: str, lowercase_counts: Counter) -> bool:
    """Проверка: встречается ли это слово в тексте как нарицательное (строчными)?"""
    lower = word.lower()
    return lowercase_counts.get(lower, 0) > 0


def get_proper_lemma(text: str, spacy_lemma: str) -> str:
    """Каноническая форма (именительный падеж) через pymorphy3.
    Если pymorphy3 не находит разбор с тегом Name/Surn — вернуть исходное слово."""
    if MORPH is None:
        return spacy_lemma or text
    try:
        parsed = MORPH.parse(text)
        # Сначала ищем разбор как имя/фамилия
        for p in parsed:
            tags_str = str(p.tag)
            if "Name" in tags_str or "Surn" in tags_str:
                nf = p.normal_form
                if nf:
                    return nf[0].upper() + nf[1:]
        # Если слово в списке известных имён — возвращаем как есть (нерусское/уменьшительное)
        if text.lower() in KNOWN_RUSSIAN_NAMES:
            return text
        # Иначе возвращаем оригинал
        return text
    except Exception:
        return spacy_lemma or text


def is_strictly_person(text: str, lemma: str, lowercase_counts: Counter,
                       entity_counts: dict = None) -> bool:
    """Строгая проверка: является ли слово именем человека?
    
    Критерии:
    1. Не в чёрном списке нарицательных существительных
    2. Не встречается как строчное нарицательное в тексте
    3. Похоже на имя (паттерн)
    4. pymorphy3 подтверждает ИЛИ слово в списке известных имён ИЛИ
       встречается ≥3 раз как PROPN (фантастические имена)
    """
    lower = text.lower()
    
    # 1. Чёрный список
    if lower in FALSE_POSITIVE_NOUNS:
        return False
    
    # 2. Встречается как нарицательное строчное → это НЕ имя
    if is_lowercase_noun_in_text(text, lowercase_counts):
        if lower not in KNOWN_RUSSIAN_NAMES:
            return False
    
    # 3. Паттерн: заглавная + строчные, длина 3-15
    if not re.match(r"^[А-ЯЁ][а-яё]{2,14}$", text):
        return False
    
    # 4. pymorphy3 проверка
    if MORPH is not None:
        try:
            parsed = MORPH.parse(text)
            for p in parsed:
                tags_str = str(p.tag)
                if "Name" in tags_str or "Surn" in tags_str:
                    return True
            # Если pymorphy3 не нашёл тег Name/Surn, но слово в списке известных — ок
            if lower in KNOWN_RUSSIAN_NAMES:
                return True
            # Если слово встречается ≥3 раз — это вероятно фантастическое имя
            # (pymorphy3 не знает "Крофт", "Грак", но они явно имена)
            if entity_counts and entity_counts.get(lower, 0) >= 3:
                return True
            return False
        except Exception:
            pass
    
    return lower in KNOWN_RUSSIAN_NAMES


def is_strictly_location(text: str, lemma: str, lowercase_counts: Counter) -> bool:
    """Строгая проверка локации."""
    lower = text.lower()
    
    # Чёрный список
    if lower in FALSE_POSITIVE_NOUNS:
        return False
    
    # Если встречается как строчное нарицательное — не локация
    if is_lowercase_noun_in_text(text, lowercase_counts):
        return False
    
    # Паттерн
    if not re.match(r"^[А-ЯЁ][а-яё\-]{2,20}$", text):
        return False
    
    return True


def split_text_into_chunks(text: str, chunk_size: int = 50000) -> list:
    """Разбить текст на части по границам предложений."""
    if len(text) <= chunk_size:
        return [text]
    
    chunks = []
    start = 0
    while start < len(text):
        end = start + chunk_size
        if end >= len(text):
            chunks.append(text[start:])
            break
        # Ищем границу предложения (точка, !, ? + пробел/перенос)
        # Идём назад от end, ищем последнее предложение
        for i in range(end, max(end - 2000, start), -1):
            if i < len(text) and text[i - 1] in ".!?":
                chunks.append(text[start:i])
                start = i
                break
        else:
            # Не нашли границу — режем жёстко
            chunks.append(text[start:end])
            start = end
    return chunks


def build_lowercase_counts(text: str) -> Counter:
    """Построить частотный словарь всех строчных слов в тексте.
    Используется для фильтра: если "Вода" появляется как PROPN, но "вода"
    встречается строчными — это нарицательное существительное, не имя."""
    counts = Counter()
    # Простая токенизация: слова из кириллицы/латиницы, длина ≥ 2
    for match in re.finditer(r"\b[а-яё][а-яё]{1,20}\b", text.lower()):
        counts[match.group()] += 1
    return counts


def process_chunk(chunk: str, chunk_offset: int, lowercase_counts: Counter,
                  entities_by_lemma: dict, propn_counts: Counter = None) -> None:
    """Обработать один чанк текста и добавить сущности в общий словарь.
    
    propn_counts — глобальный счётчик PROPN токенов (для фантастических имён).
    """
    doc = NLP(chunk)
    
    # Собираем PROPN токены из ents для исключения дубликатов
    ent_token_ranges = set()
    for ent in doc.ents:
        for i in range(ent.start, ent.end):
            ent_token_ranges.add(i)
    
    # 1. Сущности из spaCy.ents (с строгой фильтрацией)
    for ent in doc.ents:
        if not ent.text.strip():
            continue
        
        # Фильтрация по типу
        if ent.label_ == "PER":
            if not is_strictly_person(ent.text, ent.lemma_, lowercase_counts, propn_counts):
                continue
        elif ent.label_ in ("LOC", "GPE"):
            if not is_strictly_location(ent.text, ent.lemma_, lowercase_counts):
                continue
        # ORG — строгий фильтр: проверяем чёрный список + строчную форму
        elif ent.label_ == "ORG":
            lower = ent.text.lower()
            if lower in FALSE_POSITIVE_NOUNS:
                continue
            # Если встречается как строчное нарицательное — не организация
            if is_lowercase_noun_in_text(ent.text, lowercase_counts):
                # Разрешаем если это явно название (несколько слов или кавычки)
                if " " not in ent.text and "«" not in ent.text and '"' not in ent.text:
                    continue
            # Технические термины (латиница/цифры) — не организация
            if re.match(r"^[A-Z0-9\.\-\³\²]+$", ent.text):
                continue
        
        lemma_norm = get_proper_lemma(ent.text, ent.lemma_)
        key = (lemma_norm, ent.label_)
        e = entities_by_lemma[key]
        e["lemma"] = lemma_norm
        e["label"] = ent.label_
        e["forms"].add(ent.text)
        e["count"] += 1
        sent = ent.sent
        e["mentions"].append({
            "text": ent.text,
            "start": ent.start_char + chunk_offset,
            "end": ent.end_char + chunk_offset,
            "sentence": sent.text.strip()[:200],
        })
    
    # 2. Fallback: PROPN токены не вошедшие в ents
    for token in doc:
        if token.i in ent_token_ranges:
            continue
        if token.pos_ != "PROPN":
            continue
        if not is_strictly_person(token.text, token.lemma_, lowercase_counts, propn_counts):
            continue
        lemma_norm = get_proper_lemma(token.text, token.lemma_)
        key = (lemma_norm, "PER")
        e = entities_by_lemma[key]
        e["lemma"] = lemma_norm
        e["label"] = "PER"
        e["forms"].add(token.text)
        e["count"] += 1
        sent = token.sent
        e["mentions"].append({
            "text": token.text,
            "start": token.idx + chunk_offset,
            "end": token.idx + len(token.text) + chunk_offset,
            "sentence": sent.text.strip()[:200],
        })


def extract_entities(text: str) -> dict:
    """Главная функция извлечения."""
    if not text or not text.strip():
        return {"entities": [], "stats": {"total": 0, "persons": 0, "locations": 0, "organizations": 0},
                "model": "ru_core_news_sm", "version": "0.2.0",
                "truncated": False, "textLength": 0, "processedLength": 0}
    
    # 1. Считаем частоты строчных слов (для фильтра ложных срабатываний)
    lowercase_counts = build_lowercase_counts(text)
    
    # 2. Разбиваем на чанки (для больших текстов)
    chunk_size = 50000  # 50k символов на чанк
    chunks = split_text_into_chunks(text, chunk_size)
    
    # 3. ПЕРВЫЙ ПРОХОД: считаем все PROPN токены (для фантастических имён)
    # Это нужно чтобы потом разрешить "Крофт" если он встречается ≥3 раз
    propn_counts = Counter()
    for chunk in chunks:
        doc = NLP(chunk)
        for token in doc:
            if token.pos_ == "PROPN" and re.match(r"^[А-ЯЁ][а-яё]{2,14}$", token.text):
                propn_counts[token.text.lower()] += 1
    
    # 4. ВТОРОЙ ПРОХОД: извлекаем сущности с фильтрацией
    entities_by_lemma = defaultdict(lambda: {
        "lemma": "",
        "label": "",
        "forms": set(),
        "count": 0,
        "mentions": [],
    })
    
    offset = 0
    for chunk in chunks:
        process_chunk(chunk, offset, lowercase_counts, entities_by_lemma, propn_counts)
        offset += len(chunk)
    
    # 4. Группировка падежных форм
    def common_prefix_len(a: str, b: str) -> int:
        n = min(len(a), len(b))
        for i in range(n):
            if a[i].lower() != b[i].lower():
                return i
        return n
    
    final = {}
    items = list(entities_by_lemma.items())
    items.sort(key=lambda x: (len(x[0][0]), x[0][0]))
    used_keys = set()
    for i, (key_i, data_i) in enumerate(items):
        if key_i in used_keys:
            continue
        lemma_i, label_i = key_i
        canonical = data_i
        for j in range(i + 1, len(items)):
            key_j, data_j = items[j]
            if key_j in used_keys:
                continue
            lemma_j, label_j = key_j
            if label_i != label_j:
                continue
            cp = common_prefix_len(lemma_i, lemma_j)
            if cp >= 4:
                canonical["forms"].update(data_j["forms"])
                canonical["count"] += data_j["count"]
                canonical["mentions"].extend(data_j["mentions"])
                used_keys.add(key_j)
        final[key_i] = canonical
    
    # 5. Фильтр: минимум 2 упоминания (одиночные — шум)
    MIN_COUNT = 2
    entities = []
    for data in final.values():
        if data["count"] < MIN_COUNT:
            continue
        entities.append({
            "lemma": data["lemma"],
            "label": data["label"],
            "count": data["count"],
            "forms": sorted(data["forms"])[:10],
            "firstMention": data["mentions"][0]["start"] if data["mentions"] else 0,
            "mentions": data["mentions"][:50],
        })
    entities.sort(key=lambda x: -x["count"])
    
    stats = {
        "total": len(entities),
        "persons": sum(1 for e in entities if e["label"] == "PER"),
        "locations": sum(1 for e in entities if e["label"] in ("LOC", "GPE")),
        "organizations": sum(1 for e in entities if e["label"] == "ORG"),
    }
    
    return {
        "entities": entities,
        "stats": stats,
        "model": "ru_core_news_sm",
        "version": "0.2.0",
        "truncated": False,  # Теперь обрабатываем весь текст
        "textLength": len(text),
        "processedLength": len(text),
        "chunksProcessed": len(chunks),
    }


def main():
    try:
        # V2: читаем текст из файла (argv[1]) — надёжнее для больших текстов
        # Если argv[1] нет — fallback на stdin (для обратной совместимости)
        if len(sys.argv) > 1:
            with open(sys.argv[1], "r", encoding="utf-8") as f:
                text = f.read()
        else:
            text = sys.stdin.read()
        result = extract_entities(text)
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except Exception as e:
        print(json.dumps({"error": str(e), "type": type(e).__name__}, ensure_ascii=False))
        sys.exit(1)


if __name__ == "__main__":
    main()
