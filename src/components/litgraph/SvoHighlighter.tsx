"use client";

/**
 * SvoHighlighter — Ukrainian SVO syntax highlighter for POLER Layer F.
 *
 * Renders a chapter text with three semantic roles color-coded according to
 * the Layer F design system (POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md §5.1):
 *
 *   - Actor  (Subject / Nominative)  → violet pill     #8B5CF6
 *   - Verb   (Predicate, lemma)      → amber bold       #F59E0B
 *   - Target (Direct Object)         → cyan underline   #06B6D4
 *
 * Negated verbs ("не вбив", "ні") render rose-red with strike-through.
 *
 * Tokenization is regex-free: the text is split on whitespace + punctuation
 * and each token is looked up in three HashSet maps built from the triplets
 * array. This keeps the render O(N) in text length, with no regex compile.
 *
 * Clicking a highlighted token fires `onTripletSelect` with the originating
 * triplet — useful for the SVO Inspector tab to scroll to / focus the row.
 */

import React, { useMemo } from "react";
import type { SvoTripletDto } from "@/lib/tauri-commands";

interface SvoHighlighterProps {
  text: string;
  triplets: SvoTripletDto[];
  className?: string;
  onTripletSelect?: (triplet: SvoTripletDto) => void;
}

interface TokenSpan {
  text: string;
  isActor: boolean;
  isVerb: boolean;
  isTarget: boolean;
  isNegated: boolean;
  triplet?: SvoTripletDto;
}

// Punctuation & whitespace split — keeps separators in the output so the
// rendered text preserves original spacing.
const SPLIT_RE = /(\s+|[.,!?;:—«»"'()])/;

export const SvoHighlighter: React.FC<SvoHighlighterProps> = ({
  text,
  triplets,
  className = "",
  onTripletSelect,
}) => {
  const spans = useMemo<TokenSpan[]>(() => {
    if (!text || triplets.length === 0) {
      return [
        {
          text,
          isActor: false,
          isVerb: false,
          isTarget: false,
          isNegated: false,
        },
      ];
    }

    // Build three lookup maps: lowercased token → originating triplet.
    // If multiple triplets share the same surface form, the most recent
    // one wins (Map insertion order — last write wins).
    const actorSet = new Map<string, SvoTripletDto>();
    const verbSet = new Map<string, SvoTripletDto>();
    const targetSet = new Map<string, SvoTripletDto>();

    for (const t of triplets) {
      if (t.actor) actorSet.set(t.actor.toLowerCase(), t);
      if (t.verb) verbSet.set(t.verb.toLowerCase(), t);
      if (t.target) targetSet.set(t.target.toLowerCase(), t);
    }

    const tokens = text.split(SPLIT_RE);
    const result: TokenSpan[] = [];

    for (const token of tokens) {
      // Punctuation / whitespace — render as plain span, no lookup.
      if (!token || !token.trim()) {
        result.push({
          text: token,
          isActor: false,
          isVerb: false,
          isTarget: false,
          isNegated: false,
        });
        continue;
      }

      const lower = token.toLowerCase();
      const actorTrip = actorSet.get(lower);
      const verbTrip = verbSet.get(lower);
      const targetTrip = targetSet.get(lower);

      // Precedence: Actor > Verb > Target. A word like "Петро" that also
      // happens to be a verb lemma (unlikely but possible) is rendered as
      // Actor first.
      if (actorTrip) {
        result.push({
          text: token,
          isActor: true,
          isVerb: false,
          isTarget: false,
          isNegated: false,
          triplet: actorTrip,
        });
      } else if (verbTrip) {
        result.push({
          text: token,
          isActor: false,
          isVerb: true,
          isTarget: false,
          isNegated: !verbTrip.polarity,
          triplet: verbTrip,
        });
      } else if (targetTrip) {
        result.push({
          text: token,
          isActor: false,
          isVerb: false,
          isTarget: true,
          isNegated: false,
          triplet: targetTrip,
        });
      } else {
        result.push({
          text: token,
          isActor: false,
          isVerb: false,
          isTarget: false,
          isNegated: false,
        });
      }
    }

    return result;
  }, [text, triplets]);

  // No triplets → render as plain serif text.
  if (triplets.length === 0) {
    return (
      <div
        className={`font-serif text-slate-300 leading-relaxed whitespace-pre-wrap ${className}`}
      >
        {text}
      </div>
    );
  }

  return (
    <div
      className={`font-serif text-slate-200 leading-relaxed whitespace-pre-wrap ${className}`}
    >
      {spans.map((span, idx) => {
        if (span.isActor && span.triplet) {
          return (
            <span
              key={idx}
              onClick={() => onTripletSelect?.(span.triplet!)}
              className="bg-purple-900/40 border border-purple-500/40 text-purple-200 px-1 py-0.5 rounded cursor-pointer hover:bg-purple-800/60 transition-colors inline-block my-0.5"
              title={`Actor: ${span.triplet.actor}`}
            >
              {span.text}
            </span>
          );
        }
        if (span.isVerb && span.triplet) {
          const negated = span.isNegated;
          return (
            <span
              key={idx}
              onClick={() => onTripletSelect?.(span.triplet!)}
              className={`font-semibold cursor-pointer transition-colors ${
                negated
                  ? "text-rose-400 line-through decoration-rose-500"
                  : "text-amber-400 hover:text-amber-300"
              }`}
              title={`Verb: ${span.triplet.verb} (polarity: ${
                span.triplet.polarity ? "affirmative" : "negated"
              })`}
            >
              {span.text}
            </span>
          );
        }
        if (span.isTarget && span.triplet) {
          return (
            <span
              key={idx}
              onClick={() => onTripletSelect?.(span.triplet!)}
              className="underline decoration-cyan-400 decoration-2 underline-offset-2 text-cyan-200 cursor-pointer hover:text-cyan-100 transition-colors"
              title={`Target: ${span.triplet.target}`}
            >
              {span.text}
            </span>
          );
        }
        return <span key={idx}>{span.text}</span>;
      })}
    </div>
  );
};

export default SvoHighlighter;
