#!/usr/bin/env python3
"""
NER-извлечение для LitGraph v0.2.1.

Улучшения v0.2.1:
0. (NEW) Strip HTML-комментариев перед NER — избегает ложных срабатываний
   из блоков <!-- EXPECTED: ... --> в тестовых файлах корпуса
1. (NEW) Multi-token PER — поддержка полных ФИО ("Владимир Петрович Сорокин")
   через объединение смежных PROPN токенов и subtitle pattern matching
2. (NEW) Context-aware LOC whitelist — типичные локации сцены (кабинет, коридор,
   лифт, кухня, спальня) извлекаются даже в середине предложения со строчной буквы
3. Chunked processing — обрабатывает текст любого размера по частям
4. Фильтр ложных срабатываний: если "Вода" появляется как PROPN, но
   "вода" встречается как нарицательное NOUN — это НЕ персонаж
5. Минимальная частота ≥ 2 (одиночные упоминания — шум)
6. pymorphy3 строгая проверка для PER: только Name/Surn теги
7. Чёрный список распространённых ложных срабатываний

Использование:
    python3 ner_extract.py path/to/file.md
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

# === Контекстные локации: типичные места сцены, которые пишутся со строчной ===
# Эти слова извлекаются как LOC, даже если встречаются в середине предложения
# со строчной буквы. Проверка: должны встречаться ≥2 раз в тексте (не случайность).
# Решение: только если встречаются с предлогом (в/на/из/к/до/у/за/под) рядом.
CONTEXTUAL_LOCATIONS = {
    # Помещения
    "кухня", "спальня", "коридор", "кабинет", "комната", "гостиная", "прихожая",
    "ванная", "туалет", "кладовка", "подвал", "чердак", "лестница", "лестничная",
    # Городские локации
    "улица", "площадь", "переулок", "проспект", "бульвар", "набережная",
    "мост", "перекрёсток", "двор", "переход", "остановка", "станция",
    # Транспорт
    "лифт", "вагон", "машина", "автобус", "поезд", "трамвай", "троллейбус",
    # Здания
    "дом", "здание", "подъезд", "крыльцо", "крыша", "балкон", "гараж",
    # Заводские
    "завод", "цех", "склад", "офис", "магазин", "магазин", "кафе", "ресторан",
    # Учреждения
    "школа", "больница", "поликлиника", "министерство", "отделение", "управление",
    # Природа
    "берег", "поляна", "опушка", "тропа", "дорога", "шоссе", "тракт",
}

# Предлоги места — если контекстная локация встречается рядом с одним из них,
# это подтверждает что речь идёт о локации, а не о нарицательном существительном
LOCATION_PREPOSITIONS = {"в", "на", "из", "к", "до", "у", "за", "под", "по", "от"}

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
    # Для multi-token PER (ФИО) — каждый токен проверяем отдельно
    if " " in text:
        # Multi-token: проверяем каждый компонент как имя/отчество/фамилию
        parts = text.split()
        if len(parts) > 4:  # больше 4 слов — вряд ли ФИО
            return False
        for part in parts:
            if not re.match(r"^[А-ЯЁ][а-яё]{2,20}$", part):
                return False
        # Хотя бы одна часть должна быть известным именем или пройти pymorphy3
        # (полная проверка делается в extract_multitoken_persons)
        return True
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
    """Строгая проверка локации (для существительных с заглавной буквы)."""
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


def find_contextual_locations(text: str, lowercase_counts: Counter) -> list:
    """Поиск контекстных локаций (строчная форма + рядом предлог места).
    
    Возвращает список словарей: {text, lemma, start, end, sentence}.
    Это альтернативный путь для слов типа "коридор", "кабинет", "лифт",
    которые spaCy не помечает как ents, потому что они нарицательные.
    Но в контексте сцены — это реальные LOC.
    """
    results = []
    # Паттерн: предлог + опционально прилагательное + существительное из whitelist
    # Примеры: "в коридоре", "на кухню", "из кабинета", "к лифту"
    pattern = re.compile(
        r'\b(' + '|'.join(LOCATION_PREPOSITIONS) + r')\s+'
        r'(?:[а-яё]{3,20}\s+)?'  # опциональное прилагательное
        r'(' + '|'.join(CONTEXTUAL_LOCATIONS) + r')(\w*)',
        re.IGNORECASE
    )
    for match in pattern.finditer(text):
        prep = match.group(1).lower()
        noun_base = match.group(2).lower()
        noun_suffix = match.group(3) or ""
        # Полное слово: базовая форма + окончание
        full_word = noun_base + noun_suffix
        # Получаем начальную форму через pymorphy3
        lemma = noun_base.capitalize()
        if MORPH is not None:
            try:
                parsed = MORPH.parse(full_word)
                for p in parsed:
                    # Если это существительное в любом падеже — берём normal_form
                    if "NOUN" in str(p.tag) or "locy" in str(p.tag):
                        nf = p.normal_form
                        if nf:
                            lemma = nf.capitalize()
                            break
            except Exception:
                pass
        # Находим границы полной формы существительного
        start = match.start(2)
        end = match.end(2) + len(noun_suffix)
        # Контекст предложения
        sent_start = text.rfind('\n', 0, start) + 1
        sent_end = text.find('\n', end)
        if sent_end == -1:
            sent_end = min(len(text), end + 200)
        else:
            sent_end = min(sent_end, end + 200)
        sentence = text[sent_start:sent_end].strip()[:200]
        results.append({
            "text": full_word,
            "lemma": lemma,
            "start": start,
            "end": end,
            "sentence": sentence,
        })
    return results


def extract_multitoken_persons(doc, chunk_offset: int, lowercase_counts: Counter,
                                propn_counts: Counter, entities_by_lemma: dict,
                                already_covered_tokens: set) -> None:
    """Извлечение multi-token PER (ФИО, имя+отчество, имя+фамилия).
    
    Сканирует последовательности PROPN токенов, проверяет по pymorphy3,
    добавляет в entities_by_lemma как единую сущность PER.
    
    Примеры: "Владимир Петрович Сорокин", "Марина Игоревна Сергеева",
    "Алексей Викторович", "Дмитрий Петрович".
    """
    tokens = list(doc)
    i = 0
    while i < len(tokens):
        token = tokens[i]
        # Пропускаем уже покрытые spaCy.ents токены
        if token.i in already_covered_tokens:
            i += 1
            continue
        # Начинаем последовательность только с PROPN
        if token.pos_ != "PROPN":
            i += 1
            continue
        # Проверяем, что это действительно имя (не название)
        if not is_strictly_person(token.text, token.lemma_, lowercase_counts, propn_counts):
            i += 1
            continue
        
        # Собираем последовательность PROPN токенов
        sequence = [token]
        j = i + 1
        while j < len(tokens):
            next_token = tokens[j]
            # Следующий PROPN или (для отчества) Part/ADP с заглавной
            if next_token.pos_ == "PROPN" and next_token.i not in already_covered_tokens:
                sequence.append(next_token)
                j += 1
            else:
                break
        
        # Если только один PROPN — это одиночное имя, fallback обработает
        if len(sequence) < 2:
            i += 1
            continue
        
        # Проверяем валидность последовательности:
        # Хотим имя+отчество, имя+фамилия, имя+отчество+фамилия
        # Не хотим: список разных имён "Анна и Мария" (есть союз/запятая между)
        full_text = " ".join(t.text for t in sequence)
        
        # Если любой токен в последовательности в чёрном списке — пропускаем
        all_valid = True
        for t in sequence:
            if t.text.lower() in FALSE_POSITIVE_NOUNS:
                all_valid = False
                break
        if not all_valid:
            i += 1
            continue
        
        # Хотя бы один из токенов должен быть именем (не фамилией)
        has_name = False
        for t in sequence:
            if MORPH is not None:
                try:
                    for p in MORPH.parse(t.text):
                        if "Name" in str(p.tag):
                            has_name = True
                            break
                except Exception:
                    pass
            if t.text.lower() in KNOWN_RUSSIAN_NAMES:
                has_name = True
                break
        if not has_name:
            i += 1
            continue
        
        # Получаем каноническую форму: обычно последнее слово в ФИО = фамилия
        # Но для графов персонажей удобно сохранять полное ФИО как lemma
        # Если есть Name+Surn → берём нормальную форму каждого слова
        canonical_parts = []
        for t in sequence:
            part = get_proper_lemma(t.text, t.lemma_)
            canonical_parts.append(part)
        lemma_norm = " ".join(canonical_parts)
        
        # Помечаем токены как покрытые
        for t in sequence:
            already_covered_tokens.add(t.i)
        
        # Добавляем в entities_by_lemma
        key = (lemma_norm, "PER")
        e = entities_by_lemma[key]
        e["lemma"] = lemma_norm
        e["label"] = "PER"
        e["forms"].add(full_text)
        e["count"] += 1
        sent = token.sent
        e["mentions"].append({
            "text": full_text,
            "start": sequence[0].idx + chunk_offset,
            "end": sequence[-1].idx + len(sequence[-1].text) + chunk_offset,
            "sentence": sent.text.strip()[:200],
        })
        
        i = j


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
    
    # 2. Multi-token PER (новое в v0.2.1): ФИО, имя+отчество
    # Важно: запускаем ДО одиночного PROPN fallback, чтобы не задваивать
    extract_multitoken_persons(doc, chunk_offset, lowercase_counts, propn_counts,
                               entities_by_lemma, ent_token_ranges)
    
    # 3. Fallback: одиночные PROPN токены не вошедшие в ents и не в multi-token
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


def strip_html_comments(text: str) -> tuple:
    """Удалить HTML-комментарии из текста перед NER.
    
    Возвращает кортеж (cleaned_text, removed_count).
    Сохраняет длину оригинального текста, заменяя комментарии пробелами,
    чтобы не сбить offset-ы упоминаний в исходном файле.
    """
    if "<!--" not in text:
        return text, 0
    pattern = re.compile(r'<!--.*?-->', re.DOTALL)
    # Заменяем на пробелы той же длины (без переноса строк) — сохраняем offsets
    def replace_with_spaces(match):
        return ' ' * len(match.group())
    cleaned = pattern.sub(replace_with_spaces, text)
    removed = len(pattern.findall(text))
    return cleaned, removed


def extract_entities(text: str) -> dict:
    """Главная функция извлечения."""
    if not text or not text.strip():
        return {"entities": [], "stats": {"total": 0, "persons": 0, "locations": 0, "organizations": 0},
                "model": "ru_core_news_sm", "version": "0.2.1",
                "truncated": False, "textLength": 0, "processedLength": 0}
    
    # 0. (NEW v0.2.1) Удаляем HTML-комментарии (<!-- EXPECTED: ... --> и т.п.)
    # Это критично для тестовых файлов — иначе spaCy находит сущности внутри комментариев
    cleaned_text, comments_removed = strip_html_comments(text)
    
    # 1. Считаем частоты строчных слов (для фильтра ложных срабатываний)
    # Используем очищенный текст, чтобы комментарии не учитывались в частотах
    lowercase_counts = build_lowercase_counts(cleaned_text)
    
    # 2. Разбиваем на чанки (для больших текстов)
    chunk_size = 50000  # 50k символов на чанк
    chunks = split_text_into_chunks(cleaned_text, chunk_size)
    
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
    
    # 4.5 (NEW v0.2.1) Извлекаем контекстные локации (кухня, коридор, лифт)
    # Делаем это после основного прохода, чтобы не задваивать с spaCy LOC ents
    existing_loc_lemmas = {k[0].lower() for k in entities_by_lemma.keys() if k[1] in ("LOC", "GPE")}
    contextual_locs = find_contextual_locations(cleaned_text, lowercase_counts)
    for loc in contextual_locs:
        # Если уже извлечено spaCy с заглавной — не дублируем
        if loc["lemma"].lower() in existing_loc_lemmas:
            continue
        key = (loc["lemma"], "LOC")
        e = entities_by_lemma[key]
        e["lemma"] = loc["lemma"]
        e["label"] = "LOC"
        e["forms"].add(loc["text"])
        e["count"] += 1
        e["mentions"].append({
            "text": loc["text"],
            "start": loc["start"],
            "end": loc["end"],
            "sentence": loc["sentence"],
        })
    
    # 4. Группировка падежных форм (v0.4.1: ПОЛНОСТЬЮ ПЕРЕПИСАНА)
    #
    # Старая логика (v0.2.1) использовала common_prefix_len >= 4, что
    # приводило к КАТАСТРОФЕ: сущности "Архивом" + "Архисферы" + "Архитекторами"
    # сливались в одну (общий префикс "Архи"). А "Голос Бездны" + "Голос Дракона"
    # + "Голос Мира" сливались в "Голос мир". Это НЕ разные падежные формы —
    # это РАЗНЫЕ сущности, случайно имеющие общий префикс.
    #
    # Новая логика (v0.4.1):
    #   1. Single-word + Single-word: merge if cp >= 5 AND cp >= 0.6 * min_len
    #      (Алексей + Алексея → merge, Архивом + Архисферы → reject)
    #   2. Multi-word + Multi-word: merge ONLY if first word matches EXACTLY
    #      AND second word shares prefix >= 4 chars
    #      (Лорд Моретти + Лорда Моретти → merge, Голос Бездны + Голос Дракона → reject)
    #   3. Single-word + Multi-word: NEVER merge (different entity types)
    def common_prefix_len(a: str, b: str) -> int:
        n = min(len(a), len(b))
        for i in range(n):
            if a[i].lower() != b[i].lower():
                return i
        return n

    def should_merge(lemma_a: str, lemma_b: str) -> bool:
        """Решает, нужно ли сливать две сущности в одну (по лемме)."""
        a_words = lemma_a.split()
        b_words = lemma_b.split()
        # Single + Single
        if len(a_words) == 1 and len(b_words) == 1:
            cp = common_prefix_len(lemma_a, lemma_b)
            min_len = min(len(lemma_a), len(lemma_b))
            # Жёстче: prefix >= 5 символов И >= 60% длины короткого слова
            # Алексей(7) + Алексея(7): cp=6, 6/7=0.86 ✓
            # Архивом(7) + Архисферы(9): cp=4, fails 5-char threshold ✗
            # Рэй(3) + Рэя(3): cp=2, fails 5-char threshold ✗ (используем alias map)
            return cp >= 5 and cp >= 0.6 * min_len
        # Multi + Multi
        if len(a_words) >= 2 and len(b_words) >= 2:
            # Первое слово должно совпадать EXACT (case-insensitive)
            if a_words[0].lower() != b_words[0].lower():
                return False
            # Второе слово — prefix >= 4 chars (для падежей)
            cp2 = common_prefix_len(a_words[1], b_words[1])
            min_len2 = min(len(a_words[1]), len(b_words[1]))
            return cp2 >= 4 and cp2 >= 0.6 * min_len2
        # Single + Multi — NEVER
        return False

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
            if should_merge(lemma_i, lemma_j):
                canonical["forms"].update(data_j["forms"])
                canonical["count"] += data_j["count"]
                canonical["mentions"].extend(data_j["mentions"])
                used_keys.add(key_j)
        final[key_i] = canonical

    # 5. v0.4.1: Post-process — RECLASSIFY multi-word PER entities
    # по первому слову (role noun). Сюда попадают:
    #   - "Голос Бездны", "Голос Дракона" — это CONCEPT (абстракция, эпитет)
    #   - "Сфера Тепла", "Сфера Предела" — это CONCEPT
    #   - "Клан Фосфор", "Культ Хаоса", "Синдикат Экстракторов" — ORGANIZATION
    #   - "Сектор Зеркал", "Сектор Свинец" — LOCATION/SECTOR
    #   - "Лорд Моретти", "Аудитор Вэнс" — CHARACTER (но title = второе слово)
    #   - "Старик Вода", "Железная Леди" — CHARACTER (epithet, rename to last word)
    #   - "Хранитель Узора", "Хранитель Знаний" — CHARACTER (rename to second word)
    # Без этого шага — пользователю показывается 77 "персонажей" типа
    # "Голос мир" или "Архивом марта", что является смехотворным.

    # Словарь role-noun → целевой тип
    ROLE_NOUN_TO_TYPE = {
        # → concept (абстрактные эпитеты, не персонажи)
        "голос": "CONCEPT",
        "сфера": "CONCEPT",
        "бездна": "CONCEPT",
        "эхо": "CONCEPT",
        "архив": "CONCEPT",
        "свет": "CONCEPT",
        "тьма": "CONCEPT",
        "тен": "CONCEPT",  # "Тень" (несклоняемая основа)
        "тень": "CONCEPT",
        "предел": "CONCEPT",
        "пустота": "CONCEPT",
        "хаос": "CONCEPT",
        "порядок": "CONCEPT",
        "память": "CONCEPT",
        "судьба": "CONCEPT",
        "сеть": "CONCEPT",
        "кольцо": "CONCEPT",
        "фаза": "CONCEPT",
        "осада": "CONCEPT",  # событие, не персонаж
        "метрика": "CONCEPT",
        "энтропия": "CONCEPT",
        "формула": "CONCEPT",
        "принцип": "CONCEPT",
        "закон": "CONCEPT",
        "ядро": "CONCEPT",
        "узор": "CONCEPT",
        "ритм": "CONCEPT",
        "сдвиг": "CONCEPT",
        "геометрия": "CONCEPT",
        "скелет": "CONCEPT",
        "левиафан": "CONCEPT",  # мифическое существо, абстракция
        # → organization
        "клан": "ORG",
        "культ": "ORG",
        "синдикат": "ORG",
        "орден": "ORG",
        "синод": "ORG",
        "совет": "ORG",
        "братство": "ORG",
        "гильдия": "ORG",
        "корпорация": "ORG",
        "империя": "ORG",
        "королевство": "ORG",
        "княжество": "ORG",
        "республика": "ORG",
        "бухгалтерия": "ORG",
        "реестр": "ORG",
        "архитектор": "ORG",
        "институт": "ORG",
        "академия": "ORG",
        "университет": "ORG",
        "министерство": "ORG",
        "конклав": "ORG",
        "палата": "ORG",
        "фактор": "ORG",  # "Нулевой Фактор" — ORG
        "триада": "ORG",
        # → location (sector/zone)
        "сектор": "LOC",
        "зона": "LOC",
        "регион": "LOC",
        "гавань": "LOC",
        "лес": "LOC",
        "остров": "LOC",
        "башня": "LOC",
        "башни": "LOC",
        "замок": "LOC",
        # → character (epithet, rename to second word which is the actual name)
        "лорд": "PER_KEEP_LAST",
        "леди": "PER_KEEP_LAST",
        "аудитор": "PER_KEEP_LAST",
        "старик": "PER_KEEP_LAST",
        "хранитель": "PER_KEEP_LAST",
        "брокер": "PER_KEEP_LAST",
        "меняла": "PER_KEEP_LAST",
        "аспид": "PER_KEEP_LAST",
        "дракон": "PER_KEEP_LAST",
        "демон": "PER_KEEP_LAST",
        "ангел": "PER_KEEP_LAST",
        "род": "PER_KEEP_LAST",  # "Род Вэнс" → "Вэнс"
        # → reject (чистый шум — не имя, не организация)
        "платежом": "REJECT",
        "проклятием": "REJECT",
        "наследником": "REJECT",
        "сын": "REJECT",
        "дочь": "REJECT",
        "мать": "REJECT",
        "отец": "REJECT",
        "брат": "REJECT",
        "сестра": "REJECT",
        "дядя": "REJECT",
        "тётя": "REJECT",
        "тетя": "REJECT",
        "триадный": "REJECT",
        "мертвый": "REJECT",
        "мёртвый": "REJECT",
        "платиновый": "REJECT",
        "железный": "REJECT",
        "железная": "REJECT",
        "железной": "REJECT",
        "серый": "REJECT",
        "серого": "REJECT",
        "серой": "REJECT",
        "ферритовый": "REJECT",
        "нулевой": "REJECT",
        "первый": "REJECT",
        "последний": "REJECT",
        "высший": "REJECT",
        "высшего": "REJECT",
        "нижний": "REJECT",
        "нижнего": "REJECT",
        "верхний": "REJECT",
        "верхнего": "REJECT",
        "старший": "REJECT",
        "младший": "REJECT",
        "белый": "REJECT",
        "чёрный": "REJECT",
        "черный": "REJECT",
        "красный": "REJECT",
        "тёмный": "REJECT",
        "темный": "REJECT",
        "проклятый": "REJECT",
        "проклятых": "REJECT",
        "проклятое": "REJECT",
        "проклятая": "REJECT",
    }

    # v0.4.2: Слова которые ВСЕГДА отвергаются как первое слово multi-word PER
    # (числительные, прилагательные, местоимения — не могут быть началом имени)
    REJECT_FIRST_WORD_LEMMAS = {
        # Числительные
        "три", "четыре", "пять", "шесть", "семь", "восемь", "девять", "десять",
        "трое", "четверо", "пятеро",
        "первый", "второй", "третий", "четвёртый", "четвертый",
        "пятый", "шестой", "седьмой", "восьмой", "девятый", "десятый",
        "один", "одна", "одно", "две", "два",
        # Местоимения-прилагательные
        "весь", "вся", "всё", "все", "всех", "всего",
        "этот", "эта", "это", "эти", "этот",
        "тот", "та", "то", "те",
        "такой", "такая", "такое", "такие",
        "каждый", "каждая", "каждое",
        "любой", "любая", "любое",
        "сам", "сама", "само",
        "наш", "ваш", "их", "его", "её", "ее",
        "мой", "твой",
        # Прочие шумы
        "который", "которая", "которое",
        "некоторый", "никакой",
    }

    # 5.1. Применяем реклассификацию
    # v0.4.2: Определяем lemmatize_word ЗДЕСЬ (до использования в реклассификации)
    def lemmatize_word(word: str) -> str:
        """Возвращает normal_form слова через pymorphy3."""
        if MORPH is None:
            return word.lower()
        try:
            parsed = MORPH.parse(word)
            if parsed and parsed[0].normal_form:
                return parsed[0].normal_form.lower()
        except Exception:
            pass
        return word.lower()

    reclassified = {}
    for key, data in final.items():
        lemma, label = key
        # Только multi-word PER подлежат реклассификации
        if label != "PER" or " " not in lemma.strip():
            reclassified[key] = data
            continue
        words = lemma.split()
        first_word = words[0]
        first_word_lower = first_word.lower()

        # v0.4.2: Лемматизируем первое слово через pymorphy3 чтобы
        # покрыть ОТМИНЁННЫЕ формы role-noun-ов:
        #   "Сферы" → "сфера", "Культы" → "культ", "Голоса" → "голос",
        #   "Кланы" → "клан", "Бухгалтерии" → "бухгалтерия",
        #   "Братства" → "братство", "Синдиката" → "синдикат"
        first_word_lemma = first_word_lower
        if MORPH is not None:
            try:
                parsed = MORPH.parse(first_word)
                if parsed:
                    # Берём normal_form первого разбора
                    nf = parsed[0].normal_form
                    if nf:
                        first_word_lemma = nf.lower()
            except Exception:
                pass

        # Проверяем по лемматизированному первому слову
        target_type = ROLE_NOUN_TO_TYPE.get(first_word_lemma)
        if target_type is None:
            # Также проверяем оригинальную форму (для нерусских слов)
            target_type = ROLE_NOUN_TO_TYPE.get(first_word_lower)
        # v0.4.2: Жёсткий reject для числительных/прилагательных
        if target_type is None and first_word_lemma in REJECT_FIRST_WORD_LEMMAS:
            target_type = "REJECT"
        if target_type is None:
            # Не role-noun — оставляем как PER, но проверим валидность
            reclassified[key] = data
            continue
        if target_type == "REJECT":
            # Полностью отвергаем эту сущность
            continue
        if target_type == "PER_KEEP_LAST":
            # Character с титулом — берём второе слово как canonical name
            if len(words) >= 2:
                # Создаём новую сущность с lemma = второе слово
                new_lemma = words[1]
                # v0.4.2: Проверяем второе слово:
                # 1. Должно быть Capitalized, 3-20 chars
                # 2. Не должно быть в FALSE_POSITIVE_NOUNS (лемматизированное)
                # 3. Должно пройти pymorphy3 Name/Surn проверку ИЛИ быть в
                #    KNOWN_RUSSIAN_NAMES
                if len(new_lemma) < 3 or not re.match(r"^[А-ЯЁ][а-яё]{2,20}$", new_lemma):
                    continue
                # Лемматизируем для проверки по словарям
                new_lemma_nf = lemmatize_word(new_lemma) if MORPH else new_lemma.lower()
                if new_lemma_nf in FALSE_POSITIVE_NOUNS:
                    continue  # "Узор", "Шум", "Вода" — не имена
                # Проверяем pymorphy3 на Name/Surn
                is_name = False
                if MORPH is not None:
                    try:
                        for p in MORPH.parse(new_lemma):
                            if "Name" in str(p.tag) or "Surn" in str(p.tag):
                                is_name = True
                                break
                    except Exception:
                        pass
                if not is_name and new_lemma.lower() not in KNOWN_RUSSIAN_NAMES:
                    continue  # не подтверждено как имя — пропускаем
                new_key = (new_lemma, "PER")
                if new_key in reclassified:
                    # Уже есть — добавляем forms и count
                    existing = reclassified[new_key]
                    existing["forms"].update(data["forms"])
                    existing["count"] += data["count"]
                    existing["mentions"].extend(data["mentions"])
                else:
                    new_data = dict(data)
                    new_data["lemma"] = new_lemma
                    reclassified[new_key] = new_data
            continue
        # CONCEPT/ORG/LOC — меняем label
        new_key = (lemma, target_type)
        new_data = dict(data)
        new_data["label"] = target_type
        # Если уже есть с таким key — мерджим
        if new_key in reclassified:
            existing = reclassified[new_key]
            existing["forms"].update(data["forms"])
            existing["count"] += data["count"]
            existing["mentions"].extend(data["mentions"])
        else:
            reclassified[new_key] = new_data
    final = reclassified

    # 5.2. v0.4.1: Hard filter — отвергаем сущности с мусором в lemma
    def is_valid_lemma(lemma: str) -> bool:
        """Лемма должна быть чистой: только буквы, пробелы, дефис, апостроф."""
        if not lemma or not lemma.strip():
            return False
        # Newlines, tabs — недопустимы
        if "\n" in lemma or "\r" in lemma or "\t" in lemma:
            return False
        # Только буквы (кир/лат), пробелы, дефис, апостроф
        if not re.match(r"^[А-ЯЁA-Z][а-яёa-zА-ЯЁA-Z\s\-'’]+$", lemma.strip()):
            return False
        # Первый символ — заглавная буква
        if not (lemma[0].isalpha() and lemma[0].isupper()):
            return False
        # v0.4.2: Reject латино-кириллических смесей типа "Root-Оператор"
        # Если есть И кириллица, И латиница — это почти всегда шум
        has_cyr = bool(re.search(r"[А-ЯЁа-яё]", lemma))
        has_lat = bool(re.search(r"[A-Za-z]", lemma))
        if has_cyr and has_lat:
            return False
        return True

    filtered_final = {}
    for key, data in final.items():
        if not is_valid_lemma(data["lemma"]):
            continue
        filtered_final[key] = data
    final = filtered_final

    # 5.3. v0.4.2: MERGE declined forms of same multi-word CONCEPT/ORG/LOC.
    # До этого шага "Голос мир" + "Голоса мир" + "Голосом мир" — это три
    # разные CONCEPT-сущности. Сливаем их в одну.
    # lemmatize_word уже определена выше (в 5.1)

    def make_merge_key(lemma: str, label: str) -> tuple:
        """Для multi-word CONCEPT/ORG/LOC — ключ с лемматизированными словами.
        Для single-word — как есть."""
        if " " not in lemma.strip():
            return (lemma.lower(), label)
        words = lemma.split()
        # Лемматизируем каждое слово
        lemmatized = [lemmatize_word(w) for w in words]
        return (" ".join(lemmatized), label)

    # Группируем multi-word CONCEPT/ORG/LOC по merge-ключу
    merged_final = {}
    for key, data in final.items():
        lemma, label = key
        mk = make_merge_key(lemma, label)
        if mk in merged_final:
            existing = merged_final[mk]
            # Сохраняем кратчайшую исходную форму как canonical lemma
            if len(lemma) < len(existing["lemma"]):
                existing["lemma"] = lemma
            existing["forms"].update(data["forms"])
            existing["count"] += data["count"]
            existing["mentions"].extend(data["mentions"])
        else:
            merged_final[mk] = dict(data)
    final = merged_final

    # 5.4. Фильтр: минимум 3 упоминания (одиночные и двойные — шум)
    MIN_COUNT = 3

    # 5.5. v0.4.2: Post-process ORG — reject adjective+noun ORGs and Latin-only noise.
    # Применяем ЛЕММАТИЗИРОВАННОЕ первое слово для ORG тоже.
    REJECT_ORG_FIRST_WORD_LEMMAS = {
        # Прилагательные (не организация)
        "мёртвый", "мертвый", "платиновый", "золотой", "серебряный",
        "платный", "бесплатный", "открытый", "закрытый",
        "высший", "низший", "верхний", "нижний",
        "великий", "малый", "большой",
        "старый", "новый", "древний",
        "мнемарский", "имперский", "королевский",
        "триадный", "драконий",
        # Числительные
        "первый", "второй", "третий", "четвёртый", "пятый", "шестой",
        "один", "два", "три", "четыре", "пять",
        # Английские шумовые ORG-имена (UI labels, не настоящие организации)
        # — обрабатываются ниже через has_cyr проверку
    }

    def is_latin_only(s: str) -> bool:
        """True если строка содержит ТОЛЬКО латиницу (хотя бы одну букву)."""
        has_lat = bool(re.search(r"[A-Za-z]", s))
        has_cyr = bool(re.search(r"[А-ЯЁа-яё]", s))
        return has_lat and not has_cyr

    org_filtered = {}
    for key, data in final.items():
        lemma, label = key
        if label == "ORG":
            # Reject Latin-only ORGs (Force Close, Root, McWeeny, Instruments)
            # — почти всегда UI labels или tech terms, не реальные организации
            if is_latin_only(lemma):
                continue
            # Reject adjective+noun ORGs ("Мёртвый актив", "Платиновый голод")
            words = lemma.split()
            if words:
                first_word_lemma_org = lemmatize_word(words[0])
                if first_word_lemma_org in REJECT_ORG_FIRST_WORD_LEMMAS:
                    continue
            # Reject если count < 5 для ORG (строже чем PER)
            if data["count"] < 5:
                continue
        org_filtered[key] = data
    final = org_filtered

    # 5.6. v0.4.2: Post-process LOC — reject numeral+noun LOCs ("Сектор Четыре")
    loc_filtered = {}
    for key, data in final.items():
        lemma, label = key
        if label in ("LOC", "GPE") and " " in lemma.strip():
            words = lemma.split()
            if len(words) >= 2:
                second_word_lemma = lemmatize_word(words[1])
                # Если второе слово — числительное, reject
                if second_word_lemma in {"один", "два", "три", "четыре", "пять",
                                          "шесть", "семь", "восемь", "девять"}:
                    continue
        loc_filtered[key] = data
    final = loc_filtered

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
        "version": "0.2.1",
        "truncated": False,  # Теперь обрабатываем весь текст
        "textLength": len(text),
        "processedLength": len(cleaned_text),
        "commentsStripped": comments_removed,
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
