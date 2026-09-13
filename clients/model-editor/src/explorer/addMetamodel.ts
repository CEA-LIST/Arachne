/**
 * Handing one descriptor file to the replica this window is connected to, and
 * the sentence that says what that did.
 *
 * Its own module rather than part of `MetamodelBrowser.tsx` because it is the
 * behaviour and not the button: the bytes posted are the bytes read, and the
 * line that comes back is the one place the editor tells a person how far a
 * metamodel travels.
 */

import type { MetamodelListing } from '../api/types';
import type { MetamodelAddOutcome } from '../sync/useSync';

/** Handing one descriptor to the node: one file at a time, and what came back. */
export type AddState =
  | { phase: 'idle' }
  | { phase: 'sending'; file: string }
  | { phase: 'served'; line: string }
  | { phase: 'refused'; detail: string };

/** What to call a language in a sentence: its package, or its nsURI when it declares none. */
function languageName(listing: MetamodelListing): string {
  return listing.package.length > 0 ? listing.package : listing.nsURI;
}

/**
 * The line a successful add leaves under the control, and why it is worded
 * like that.
 *
 * Posting a descriptor reaches ONE replica. Bob, next door, goes on listing
 * the metamodels he was started with, and will go on listing exactly those
 * until a model written in this language arrives there — the descriptor
 * travels inside the `Install` operation that opened that model's log, and by
 * no other route. A person who reads "added" here and then finds the second
 * editor window unchanged concludes the add failed, so the line states the
 * reach and the route in one breath instead of leaving that to be discovered.
 */
export function servedHereLine(listing: MetamodelListing, added: boolean): string {
  const name = languageName(listing);
  if (!added) {
    return `This replica already served ${name} — the file hashes to the digest it holds, so nothing changed.`;
  }
  return `This replica serves ${name} now; another replica learns it only when a model written in ${name} reaches it.`;
}

/**
 * Read one chosen file and hand its text to the node, as read.
 *
 * The bytes posted are the bytes read: the node parses the body itself and
 * takes the metamodel's identity over what it parsed, so a re-serialization
 * of a parsed object would be a descriptor nobody wrote and a digest nobody
 * can name.
 */
export async function addDescriptorFile(
  file: File,
  add: (text: string, fileName: string) => Promise<MetamodelAddOutcome>,
): Promise<AddState> {
  let text: string;
  try {
    text = await file.text();
  } catch (err) {
    return {
      phase: 'refused',
      detail: `could not read ${file.name}: ${err instanceof Error ? err.message : String(err)}`,
    };
  }
  const outcome = await add(text, file.name);
  if (outcome.listing === null || !outcome.ok) {
    return { phase: 'refused', detail: outcome.detail ?? 'the node refused the descriptor without saying why' };
  }
  return { phase: 'served', line: servedHereLine(outcome.listing, outcome.added) };
}
