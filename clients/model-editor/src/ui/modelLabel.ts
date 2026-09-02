/**
 * What a model is called in a tab and in the list: the package of the
 * metamodel it is bound to, then the first eight characters of its id, which
 * is what a person reads out to a colleague. Pure, so the label rule is one
 * function for the strip, the list and the top bar.
 */

import type { HostedModel, MetamodelId, MetamodelListing, ModelId } from '../api/types';
import type { ModelTab } from '../state/store';

/** The label for the default log, which is bound to nothing. */
export const DEFAULT_LOG_LABEL = 'default log';

/** Eight characters of an id: enough to tell two models apart by eye, short enough for a tab. */
export function shortId(id: ModelId): string {
  return id.slice(0, 8);
}

/** The package name for a metamodel the node lists, or its nsURI when the listing does not carry it, or null when unknown. */
export function packageOf(id: MetamodelId | null, metamodels: readonly MetamodelListing[]): string | null {
  if (id === null) return null;
  const listed = metamodels.find((entry) => entry.digest === id.digest);
  if (listed !== undefined && listed.package.length > 0) return listed.package;
  return id.nsURI.length > 0 ? id.nsURI : null;
}

/** The package word for a hosted model: from its tab's descriptor, else from the listing, else "default log" for the unbound one. */
export function modelPackage(
  id: ModelId,
  tab: ModelTab | undefined,
  hosted: readonly HostedModel[] | null,
  metamodels: readonly MetamodelListing[],
): string {
  if (tab?.metamodel) return tab.metamodel.package;
  const entry = hosted?.find((model) => model.modelId === id);
  if (entry !== undefined && entry.metamodelId === null) return DEFAULT_LOG_LABEL;
  return packageOf(entry?.metamodelId ?? null, metamodels) ?? 'model';
}

/** `<package> · <id8>`, the tab's name. */
export function modelLabel(
  id: ModelId,
  tab: ModelTab | undefined,
  hosted: readonly HostedModel[] | null,
  metamodels: readonly MetamodelListing[],
): string {
  return `${modelPackage(id, tab, hosted, metamodels)} · ${shortId(id)}`;
}
