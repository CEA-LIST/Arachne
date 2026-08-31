/**
 * The single import point for icons.
 *
 * Everything in the UI imports glyphs from here, never from `lucide-react`
 * directly, so the whole set can be swapped for inline SVGs in ONE file if the
 * dependency audit ever sours. lucide-react is `sideEffects: false` with an
 * ESM entry, so Rollup tree-shakes this to exactly the glyphs used.
 *
 * Icon choices are keyed on descriptor *kinds* (containment vs reference,
 * abstract vs concrete, isId), never on class names from any metamodel.
 */

export {
  ArrowDown,
  ArrowUp,
  Box,
  Braces,
  Check,
  ChevronDown,
  ChevronRight,
  ChevronsDownUp,
  CircleAlert,
  CircleHelp,
  Copy,
  Cpu,
  Download,
  FileWarning,
  Folder,
  FolderOpen,
  Key,
  Keyboard,
  Link2,
  MousePointerClick,
  Plug,
  Plus,
  RefreshCw,
  Search,
  Terminal,
  Trash2,
  TriangleAlert,
  Unlink,
  X,
} from 'lucide-react';

/** House defaults: 16px, stroke 1.5 — the IDE register. */
export const ICON: { size: number; strokeWidth: number } = { size: 16, strokeWidth: 1.5 };

/** Slightly larger, for empty-state cards. */
export const ICON_LG: { size: number; strokeWidth: number } = { size: 32, strokeWidth: 1.25 };
