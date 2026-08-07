/**
 * NER — Named Entity Recognition для LitGraph.
 *
 * Типы (зеркало Rust struct'ов из src-tauri/src/commands/ner.rs).
 */

export type EntityLabel = "PER" | "LOC" | "GPE" | "ORG" | "MISC";

export interface EntityMention {
  text: string;
  start: number;
  end: number;
  sentence: string;
}

export interface Entity {
  lemma: string;
  label: EntityLabel;
  count: number;
  forms: string[];
  firstMention: number;
  mentions: EntityMention[];
}

export interface NerStats {
  total: number;
  persons: number;
  locations: number;
  organizations: number;
}

export interface NerResult {
  entities: Entity[];
  stats: NerStats;
  model: string;
  version: string;
  truncated: boolean;
  textLength: number;
  processedLength: number;
}

export interface NerError {
  error: string;
}

// Цвета по типам сущностей (как в spaCy displaCy)
export const ENTITY_LABELS: Record<EntityLabel, { ru: string; color: string; description: string }> = {
  PER: {
    ru: "Персона",
    color: "#aa6633",
    description: "Имена людей, фамилии, прозвища",
  },
  LOC: {
    ru: "Локация",
    color: "#3366aa",
    description: "Географические объекты, города, страны",
  },
  GPE: {
    ru: "Гео-полит.",
    color: "#339966",
    description: "Страны, государства, регионы",
  },
  ORG: {
    ru: "Организация",
    color: "#993366",
    description: "Компании, учреждения, группы",
  },
  MISC: {
    ru: "Прочее",
    color: "#666666",
    description: "События, произведения, даты",
  },
};
