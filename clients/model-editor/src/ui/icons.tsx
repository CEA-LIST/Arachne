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
  CircleAlert,
  CircleHelp,
  Copy,
  Cpu,
  Download,
  FileWarning,
  Folder,
  FolderOpen,
  Hourglass,
  Key,
  Keyboard,
  Link2,
  // Not ChevronsDownUp: two chevrons meeting render as an ✕ at 16px, and an ✕
  // beside a filter field reads as "clear the filter".
  ListCollapse,
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

export type { LucideIcon } from 'lucide-react';
