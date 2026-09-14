/**
 * Which metamodel a join names, from what the Join by id form was given.
 *
 * Its own module rather than part of `ModelsPanel.tsx` because it is the
 * behaviour and not the form: the two ways of naming a metamodel differ in
 * exactly one thing, whether this replica already holds a descriptor for it,
 * and that difference is the whole point of the second one.
 *
 * # Why a digest can be typed at all
 *
 * The dropdown is fed by this window's own `GET /api/metamodels`, so it can
 * only ever offer what this replica already serves. That is right for
 * creating a model — a create is written from the descriptor, and a node with
 * no descriptor has nothing to write the opening operation from — and wrong
 * for joining one. A join writes nothing: the node hosts the id with the
 * binding left *pending*, asks its peers for the log, and takes the
 * descriptor out of the model's own first operation when it arrives. So a
 * replica that has never seen a language can join a model written in it, and
 * the dropdown was the only thing standing in the way.
 *
 * # Why the dropdown has no default any more
 *
 * It used to read an empty choice as the listing's first entry, which is what
 * the `<select>` happens to show. That turned "I did not answer this" into an
 * answer, and the answer was a language picked by list order. Rehearsing the
 * demo on 2026-09-14, a model written in `library` was joined as
 * `behaviortree` that way, on a replica that held `behaviortree` and not
 * `library`: the node took the binding without a word, because it holds the
 * language that was named, and went on serving that language's descriptor for
 * a document that was not written in it. An empty dropdown is a refusal here
 * now, and the form says which control to reach for instead.
 *
 * # What is checked here and what is not
 *
 * The shape, and nothing else. A digest that names a metamodel no replica in
 * the session holds, or one that does not match the model's own history, is
 * refused by the node — a join is hosted and stays pending, and a local write
 * into it comes back with the node's sentence naming the digest the model was
 * joined under. Answering that here from the local listing would be guessing,
 * and would refuse the case the field was added for.
 */

import type { MetamodelId, MetamodelListing } from '../api/types';
import { isMetamodelDigest } from '../model/digest';

/** How the Join by id form was told which metamodel the model is bound to. */
export type BoundTo =
  /** The dropdown: a digest this replica holds; empty until one is chosen. */
  | { source: 'held'; digest: string }
  /** The text field: a digest typed or pasted, which this replica need not hold. */
  | { source: 'digest'; digest: string };

/** Either the identity to register the join under, or the sentence to print instead. */
export type JoinTarget = { ok: true; metamodelId: MetamodelId } | { ok: false; error: string };

/**
 * The `metamodel_id` for a join, or why the form cannot send one.
 *
 * The node reads only the digest out of a registration's `metamodel_id` and
 * opens the log from the descriptor that digest names, so the `nsURI` beside
 * it is a label. It is filled in from the listing when this replica happens
 * to hold the digest, and left empty when it does not, which is the honest
 * answer: the nsURI arrives with the descriptor, inside the model.
 */
export function joinTarget(bound: BoundTo, metamodels: readonly MetamodelListing[]): JoinTarget {
  if (bound.source === 'held') {
    const entry = metamodels.find((candidate) => candidate.digest === bound.digest);
    if (entry === undefined) {
      return { ok: false, error: 'choose the language this model is written in, or give its digest' };
    }
    return { ok: true, metamodelId: { nsURI: entry.nsURI, digest: entry.digest } };
  }
  const digest = bound.digest.trim();
  if (!isMetamodelDigest(digest)) {
    return { ok: false, error: 'a metamodel digest is 64 lowercase hex characters' };
  }
  const held = metamodels.find((candidate) => candidate.digest === digest);
  return { ok: true, metamodelId: { nsURI: held?.nsURI ?? '', digest } };
}
