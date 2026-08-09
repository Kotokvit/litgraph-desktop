//! Временная модель reasoning engine.
//!
//! `TemporalAnchor` — точка в нарративном времени. Состоит из:
//!   - `chapter_num` (u32) — числовая часть главы (12 для «Глава 12», 28 для «Глава 28б»)
//!   - `chapter_suffix` (Option<String>) — суффикс суб-главы («б», «в», «г»)
//!   - `scene_index` (Option<u32>) — индекс сцены внутри главы
//!   - `char_offset` (usize) — байтовое смещение в исходном тексте
//!
//! Порядок сравнения (см. SPEC §2.2):
//!   chapter_num (numeric) → chapter_suffix (None < Some, лексикографически)
//!   → scene_index (None < Some, numeric) → char_offset (numeric).
//!
//! `TimeInterval` — замкнутый интервал `[start, end]` на оси `TemporalAnchor`.
//! `Timeline` — упорядоченное множество глав нарратива с курсором «текущая позиция».
//!
//! Модуль фундаментальный: не имеет upward-зависимостей от других reasoning-модулей.
//! Использует только `serde` для (де)сериализации.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Точка в нарративном времени. SPEC §2.2.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TemporalAnchor {
    /// Числовая часть главы (12 для «Глава 12», 28 для «Глава 28б»).
    pub chapter_num: u32,
    /// Суффикс суб-главы: `Some("б")` для «Глава 28б», `None` для обычной главы.
    pub chapter_suffix: Option<String>,
    /// Индекс сцены внутри главы (`None` если сцены не выделены).
    pub scene_index: Option<u32>,
    /// Байтовое смещение в исходном тексте главы.
    pub char_offset: usize,
}

impl TemporalAnchor {
    /// Минимальный anchor для главы: offset 0, без суффикса, без сцены.
    pub fn new(chapter_num: u32) -> Self {
        Self {
            chapter_num,
            chapter_suffix: None,
            scene_index: None,
            char_offset: 0,
        }
    }

    /// Anchor для суб-главы: «Глава 28б» → `with_suffix(28, "б")`.
    pub fn with_suffix(chapter_num: u32, suffix: impl Into<String>) -> Self {
        Self {
            chapter_num,
            chapter_suffix: Some(suffix.into()),
            scene_index: None,
            char_offset: 0,
        }
    }

    /// Builder: установить индекс сцены.
    pub fn with_scene(mut self, scene_index: u32) -> Self {
        self.scene_index = Some(scene_index);
        self
    }

    /// Builder: установить байтовое смещение.
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.char_offset = offset;
        self
    }

    /// `true` если `self` происходит строго раньше `other`.
    pub fn before(&self, other: &TemporalAnchor) -> bool {
        self < other
    }

    /// `true` если `self` происходит строго позже `other`.
    pub fn after(&self, other: &TemporalAnchor) -> bool {
        self > other
    }

    /// `true` если `chapter_num` и `chapter_suffix` совпадают.
    /// Сцена и смещение игнорируются — это позиция внутри одной главы.
    pub fn same_chapter(&self, other: &TemporalAnchor) -> bool {
        self.chapter_num == other.chapter_num
            && self.chapter_suffix == other.chapter_suffix
    }

    /// `self <= other`.
    pub fn same_or_before(&self, other: &TemporalAnchor) -> bool {
        self <= other
    }

    /// `self >= other`.
    pub fn same_or_after(&self, other: &TemporalAnchor) -> bool {
        self >= other
    }

    /// Человекочитаемое название главы на русском: «Глава 12», «Глава 28б».
    pub fn display_chapter(&self) -> String {
        match &self.chapter_suffix {
            Some(s) => format!("Глава {}{}", self.chapter_num, s),
            None => format!("Глава {}", self.chapter_num),
        }
    }

    /// Алиас для `new` — подчёркивает «строим anchor из номера главы».
    pub fn from_chapter_num(num: u32) -> Self {
        Self::new(num)
    }

    /// Sentinel: самая ранняя возможная позиция (Глава 0, offset 0).
    /// Используется как «до начала нарратива».
    pub fn earliest() -> Self {
        Self {
            chapter_num: 0,
            chapter_suffix: None,
            scene_index: None,
            char_offset: 0,
        }
    }

    /// Вернуть более поздний из двух anchor'ов.
    ///
    /// NB: из-за конфликта имён с `Ord::max` (трейт-метод с сигнатурой
    /// `fn max(self, other: Self) -> Self`) вызов через dot-синтаксис
    /// `a.max(&b)` резолвится в `Ord::max`. Чтобы вызвать этот метод,
    /// используйте UFCS: `TemporalAnchor::max(&a, &b)`, либо явный
    /// reference-receiver: `(&a).max(&b)`.
    pub fn max(&self, other: &TemporalAnchor) -> TemporalAnchor {
        if self >= other {
            self.clone()
        } else {
            other.clone()
        }
    }
}

// ===== Ручная реализация Ord/PartialOrd по правилу SPEC §2.2 =====
//
// Поля сравниваются в порядке:
//   1. chapter_num (numeric)
//   2. chapter_suffix (None < Some, лексикографически внутри Some)
//   3. scene_index (None < Some, numeric внутри Some)
//   4. char_offset (numeric)
//
// `Option<T>: Ord` по умолчанию даёт `None < Some(_)`, что соответствует
// правилу. Реализация написана явно (а не через derive) для самодокументируемости
// и устойчивости к будущим изменениям набора полей.
//
// Консистентность с производными `PartialEq`/`Eq`/`Hash`: ручной `Ord`
// сравнивает ВСЕ поля в том же порядке, в каком их сравнивает производный
// `PartialEq`, поэтому `a.cmp(b) == Equal ⟺ a == b`. Производный `Hash`
// хэширует все поля, что согласовано с `Eq`.

impl PartialOrd for TemporalAnchor {
    fn partial_cmp(&self, other: &TemporalAnchor) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TemporalAnchor {
    fn cmp(&self, other: &TemporalAnchor) -> Ordering {
        // 1. chapter_num (numeric).
        match self.chapter_num.cmp(&other.chapter_num) {
            Ordering::Equal => {}
            ord => return ord,
        }
        // 2. chapter_suffix (None < Some, лексикографически).
        let suffix_cmp = match (&self.chapter_suffix, &other.chapter_suffix) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) => a.cmp(b),
        };
        match suffix_cmp {
            Ordering::Equal => {}
            ord => return ord,
        }
        // 3. scene_index (None < Some, numeric).
        let scene_cmp = match (self.scene_index, other.scene_index) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) => a.cmp(&b),
        };
        match scene_cmp {
            Ordering::Equal => {}
            ord => return ord,
        }
        // 4. char_offset (numeric).
        self.char_offset.cmp(&other.char_offset)
    }
}

/// Замкнутый временной интервал `[start, end]` на оси `TemporalAnchor`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeInterval {
    pub start: TemporalAnchor,
    pub end: TemporalAnchor,
}

impl TimeInterval {
    /// Создать интервал. В debug-сборке assert'ит, что `start <= end`.
    pub fn new(start: TemporalAnchor, end: TemporalAnchor) -> Self {
        debug_assert!(
            start <= end,
            "TimeInterval::new: start ({:?}) должен быть <= end ({:?})",
            start,
            end
        );
        Self { start, end }
    }

    /// `true` если точка лежит внутри `[start, end]` (включая границы).
    pub fn contains(&self, point: &TemporalAnchor) -> bool {
        &self.start <= point && point <= &self.end
    }

    /// `true` если интервалы имеют общую точку (замкнутые интервалы).
    /// `[a1, a2] ∩ [b1, b2] ≠ ∅ ⟺ a1 <= b2 && b1 <= a2`.
    pub fn overlaps(&self, other: &TimeInterval) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    /// Длительность в главах: `end.chapter_num - start.chapter_num`.
    ///
    /// Особый случай: если `chapter_num` равны, но суффиксы различаются
    /// (напр. 28 → 28б), считаем как +1 (суб-главы считаются отдельными шагами).
    pub fn duration_chapters(&self) -> u32 {
        if self.end.chapter_num == self.start.chapter_num {
            // Та же числовая глава. Если суффиксы разные — соседние суб-главы,
            // считаем как 1 шаг.
            if self.end.chapter_suffix != self.start.chapter_suffix {
                1
            } else {
                0
            }
        } else {
            // Разные числовые главы: простая разность (с насыщением до 0).
            self.end
                .chapter_num
                .saturating_sub(self.start.chapter_num)
        }
    }
}

/// Упорядоченное множество глав нарратива + курсор «текущая позиция».
///
/// Главы хранятся без дубликатов, отсортированными по `Ord` `TemporalAnchor`.
/// Курсор указывает на текущую позицию в нарративе (изменяется через `advance_to`).
pub struct Timeline {
    /// Упорядоченные главы (без дубликатов, отсортированы).
    chapters: Vec<TemporalAnchor>,
    /// Текущая позиция в нарративе.
    cursor: TemporalAnchor,
}

impl Timeline {
    /// Пустой timeline; курсор = `earliest()`.
    pub fn new() -> Self {
        Self {
            chapters: Vec::new(),
            cursor: TemporalAnchor::earliest(),
        }
    }

    /// Построить timeline из номеров глав (без суффиксов).
    /// Дубликаты удаляются, результат сортируется по возрастанию.
    /// Курсор остаётся в позиции `earliest()` — «нарратив ещё не начат».
    pub fn from_chapters<I: IntoIterator<Item = u32>>(chapter_nums: I) -> Self {
        let mut chapters: Vec<TemporalAnchor> = chapter_nums
            .into_iter()
            .map(TemporalAnchor::from_chapter_num)
            .collect();
        chapters.sort();
        chapters.dedup();
        Self {
            chapters,
            cursor: TemporalAnchor::earliest(),
        }
    }

    /// Вставить главу в отсортированную позицию, дедуплицировать.
    /// Курсор не модифицируется.
    pub fn add_chapter(&mut self, anchor: TemporalAnchor) {
        match self.chapters.binary_search(&anchor) {
            Ok(_) => { /* уже есть — пропускаем */ }
            Err(pos) => self.chapters.insert(pos, anchor),
        }
    }

    /// Срез всех глав (отсортированный, без дубликатов).
    pub fn chapters(&self) -> &[TemporalAnchor] {
        &self.chapters
    }

    /// Количество глав.
    pub fn chapter_count(&self) -> usize {
        self.chapters.len()
    }

    /// Индекс главы в отсортированном списке (по полному `Ord`).
    /// Возвращает `None`, если anchor не найден среди chapters.
    pub fn position_of(&self, anchor: &TemporalAnchor) -> Option<usize> {
        self.chapters.binary_search(anchor).ok()
    }

    /// Продвинуть курсор к `anchor`. Ошибка, если anchor не в timeline.
    /// В случае ошибки курсор не изменяется.
    pub fn advance_to(&mut self, anchor: &TemporalAnchor) -> Result<(), String> {
        match self.position_of(anchor) {
            Some(_) => {
                self.cursor = anchor.clone();
                Ok(())
            }
            None => Err(format!(
                "advance_to: anchor {:?} ({}) не найден в timeline",
                anchor,
                anchor.display_chapter()
            )),
        }
    }

    /// Текущая позиция в нарративе.
    pub fn cursor(&self) -> &TemporalAnchor {
        &self.cursor
    }

    /// Следующая глава после курсора (строго больше по `Ord`).
    /// `None` если курсор на последней главе или после неё.
    pub fn next_chapter(&self) -> Option<&TemporalAnchor> {
        self.chapters.iter().find(|c| **c > self.cursor)
    }

    /// Предыдущая глава перед курсором (строго меньше по `Ord`).
    /// `None` если курсор на первой главе или до неё.
    pub fn previous_chapter(&self) -> Option<&TemporalAnchor> {
        self.chapters.iter().rev().find(|c| **c < self.cursor)
    }

    /// Главы в интервале `[from, to]` (включая границы), отсортированные.
    /// Возвращает пустой вектор, если `from > to`.
    pub fn chapters_between(
        &self,
        from: &TemporalAnchor,
        to: &TemporalAnchor,
    ) -> Vec<&TemporalAnchor> {
        if from > to {
            return Vec::new();
        }
        self.chapters
            .iter()
            .filter(|c| from <= *c && *c <= to)
            .collect()
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

// ============ Юнит-тесты ============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_anchor_ordering_basic() {
        let a = TemporalAnchor::new(5);
        let b = TemporalAnchor::new(12);
        assert!(a < b, "Глава 5 < Глава 12");
        assert!(a.before(&b));
        assert!(b.after(&a));
        assert!(a.same_or_before(&b));
        assert!(b.same_or_after(&a));

        // char_offset внутри одной главы.
        let a_off = TemporalAnchor::new(12).with_offset(100);
        let b_off = TemporalAnchor::new(12).with_offset(500);
        assert!(a_off < b_off, "Та же глава, меньший offset < больший");
        assert!(a_off.same_chapter(&b_off));

        // scene_index внутри одной главы.
        let s0 = TemporalAnchor::new(12).with_scene(0);
        let s1 = TemporalAnchor::new(12).with_scene(1);
        assert!(s0 < s1, "scene 0 < scene 1");
        assert!(!s0.after(&s1));

        // None < Some для scene_index.
        let no_scene = TemporalAnchor::new(12);
        assert!(no_scene < s0, "Без сцены < с сценой 0");
    }

    #[test]
    fn test_subchapter_suffix_ordering() {
        // 28 < 28б < 28в
        let c28 = TemporalAnchor::new(28);
        let c28_b = TemporalAnchor::with_suffix(28, "б");
        let c28_v = TemporalAnchor::with_suffix(28, "в");

        assert!(c28 < c28_b, "Глава 28 < Глава 28б (None < Some)");
        assert!(c28_b < c28_v, "Глава 28б < Глава 28в (лексикографически)");
        assert!(c28 < c28_v, "Глава 28 < Глава 28в (транзитивно)");

        // same_chapter: 28 и 28б — РАЗНЫЕ главы.
        assert!(!c28.same_chapter(&c28_b));
        // 28б и 28б — та же.
        assert!(c28_b.same_chapter(&c28_b.clone()));

        // display_chapter.
        assert_eq!(c28.display_chapter(), "Глава 28");
        assert_eq!(c28_b.display_chapter(), "Глава 28б");
        assert_eq!(c28_v.display_chapter(), "Глава 28в");
    }

    #[test]
    fn test_same_chapter_detection() {
        let a = TemporalAnchor::new(15).with_scene(2).with_offset(1000);
        let b = TemporalAnchor::new(15).with_offset(50);
        // Та же глава (15, без суффикса), несмотря на разные scene/offset.
        assert!(a.same_chapter(&b));

        let c = TemporalAnchor::with_suffix(15, "а");
        assert!(!a.same_chapter(&c), "15 и 15а — разные главы");

        // Суффиксы «а» и «б» — разные главы.
        let c_a = TemporalAnchor::with_suffix(15, "а");
        let c_b = TemporalAnchor::with_suffix(15, "б");
        assert!(!c_a.same_chapter(&c_b));
    }

    #[test]
    fn test_time_interval_contains_and_overlaps() {
        let start = TemporalAnchor::new(5);
        let end = TemporalAnchor::new(10);
        let interval = TimeInterval::new(start.clone(), end.clone());

        // contains — границы включены.
        assert!(interval.contains(&TemporalAnchor::new(5)));
        assert!(interval.contains(&TemporalAnchor::new(10)));
        assert!(interval.contains(&TemporalAnchor::new(7)));
        assert!(!interval.contains(&TemporalAnchor::new(4)));
        assert!(!interval.contains(&TemporalAnchor::new(11)));

        // overlaps — пересекающиеся интервалы.
        let other = TimeInterval::new(TemporalAnchor::new(8), TemporalAnchor::new(12));
        assert!(interval.overlaps(&other), "[5,10] ∩ [8,12] ≠ ∅");

        // overlaps — непересекающиеся.
        let disjoint = TimeInterval::new(TemporalAnchor::new(11), TemporalAnchor::new(15));
        assert!(!interval.overlaps(&disjoint), "[5,10] ∩ [11,15] = ∅");

        // Закрытые границы: касающиеся интервалы пересекаются.
        let touching = TimeInterval::new(TemporalAnchor::new(10), TemporalAnchor::new(12));
        assert!(
            interval.overlaps(&touching),
            "Закрытые границы: [5,10] ∩ [10,12] содержит 10"
        );

        // duration_chapters.
        assert_eq!(interval.duration_chapters(), 5, "10 - 5 = 5");
        let zero = TimeInterval::new(TemporalAnchor::new(7), TemporalAnchor::new(7));
        assert_eq!(zero.duration_chapters(), 0, "Та же глава = 0");
        let subch = TimeInterval::new(
            TemporalAnchor::new(28),
            TemporalAnchor::with_suffix(28, "б"),
        );
        assert_eq!(
            subch.duration_chapters(),
            1,
            "28 → 28б: разные суффиксы = +1"
        );
    }

    #[test]
    fn test_timeline_add_chapter_sorted() {
        let mut tl = Timeline::new();
        tl.add_chapter(TemporalAnchor::new(10));
        tl.add_chapter(TemporalAnchor::new(3));
        tl.add_chapter(TemporalAnchor::new(7));
        tl.add_chapter(TemporalAnchor::new(3)); // дубликат
        tl.add_chapter(TemporalAnchor::with_suffix(7, "а")); // суб-глава

        assert_eq!(tl.chapter_count(), 4, "Дубликат 3 должен быть удалён");

        // Ожидаемый порядок: 3, 7, 7а, 10.
        assert_eq!(tl.chapters()[0], TemporalAnchor::new(3));
        assert_eq!(tl.chapters()[1], TemporalAnchor::new(7));
        assert_eq!(tl.chapters()[2], TemporalAnchor::with_suffix(7, "а"));
        assert_eq!(tl.chapters()[3], TemporalAnchor::new(10));

        // Курсор не должен измениться от add_chapter.
        assert_eq!(*tl.cursor(), TemporalAnchor::earliest());
    }

    #[test]
    fn test_timeline_chapters_between() {
        let tl = Timeline::from_chapters(vec![1, 3, 5, 7, 9, 12]);
        let from = TemporalAnchor::new(3);
        let to = TemporalAnchor::new(9);
        let between = tl.chapters_between(&from, &to);
        let nums: Vec<u32> = between.iter().map(|c| c.chapter_num).collect();
        assert_eq!(nums, vec![3, 5, 7, 9], "Включая границы");

        // from > to → пусто.
        let empty = tl.chapters_between(&TemporalAnchor::new(9), &TemporalAnchor::new(3));
        assert!(empty.is_empty());

        // position_of.
        assert_eq!(tl.position_of(&TemporalAnchor::new(5)), Some(2));
        assert_eq!(tl.position_of(&TemporalAnchor::new(100)), None);

        // from_chapters дедуплицирует и сортирует.
        let nums: Vec<u32> = tl.chapters().iter().map(|c| c.chapter_num).collect();
        assert_eq!(nums, vec![1, 3, 5, 7, 9, 12]);
    }

    #[test]
    fn test_advance_to_rejects_unknown_anchor() {
        let mut tl = Timeline::from_chapters(vec![1, 2, 3]);

        // Известный anchor — OK.
        let res = tl.advance_to(&TemporalAnchor::new(2));
        assert!(res.is_ok());
        assert_eq!(*tl.cursor(), TemporalAnchor::new(2));

        // next/prev относительно курсора (2).
        assert_eq!(
            tl.next_chapter().map(|c| c.chapter_num),
            Some(3),
            "После 2 идёт 3"
        );
        assert_eq!(
            tl.previous_chapter().map(|c| c.chapter_num),
            Some(1),
            "Перед 2 идёт 1"
        );

        // Unknown anchor — Err, курсор не меняется.
        let res = tl.advance_to(&TemporalAnchor::new(99));
        assert!(res.is_err(), "Неизвестный anchor должен быть отклонён");
        assert_eq!(
            *tl.cursor(),
            TemporalAnchor::new(2),
            "Курсор не должен измениться при ошибке"
        );

        // next_chapter на последней = None.
        tl.advance_to(&TemporalAnchor::new(3)).unwrap();
        assert!(tl.next_chapter().is_none(), "После последней главы нет next");

        // previous_chapter на первой = None.
        tl.advance_to(&TemporalAnchor::new(1)).unwrap();
        assert!(
            tl.previous_chapter().is_none(),
            "Перед первой главой нет prev"
        );
    }

    #[test]
    fn test_from_chapters_dedup_and_sort() {
        let tl = Timeline::from_chapters(vec![5, 1, 3, 5, 1, 9]);
        let nums: Vec<u32> = tl.chapters().iter().map(|c| c.chapter_num).collect();
        assert_eq!(nums, vec![1, 3, 5, 9]);
        assert_eq!(tl.chapter_count(), 4);
        // Курсор = earliest (нарратив ещё не начат).
        assert_eq!(*tl.cursor(), TemporalAnchor::earliest());
        // next_chapter = первая глава.
        assert_eq!(tl.next_chapter().map(|c| c.chapter_num), Some(1));
        // previous_chapter = None (нет глав раньше earliest).
        assert!(tl.previous_chapter().is_none());
    }

    #[test]
    fn test_max_and_earliest() {
        let a = TemporalAnchor::new(3);
        let b = TemporalAnchor::new(8);
        // Внимание: используем UFCS, т.к. dot-синтаксис `a.max(&b)` резолвится
        // в трейт-метод `Ord::max`. См. doc-comment на `TemporalAnchor::max`.
        assert_eq!(TemporalAnchor::max(&a, &b), b);
        assert_eq!(TemporalAnchor::max(&b, &a), b);
        assert_eq!(TemporalAnchor::max(&a, &a), a);

        let earliest = TemporalAnchor::earliest();
        assert!(earliest < a, "earliest < любой другой anchor");
        assert_eq!(earliest.chapter_num, 0);
        assert_eq!(earliest.char_offset, 0);

        // max с earliest всегда возвращает другой.
        assert_eq!(TemporalAnchor::max(&earliest, &a), a);
    }

    #[test]
    fn test_default_trait() {
        let tl = Timeline::default();
        assert_eq!(tl.chapter_count(), 0);
        assert_eq!(*tl.cursor(), TemporalAnchor::earliest());
        assert!(tl.chapters().is_empty());
    }
}
