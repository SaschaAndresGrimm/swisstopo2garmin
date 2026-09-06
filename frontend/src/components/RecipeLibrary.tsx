import { useCallback, useEffect, useState } from "react";
import { api } from "../state/api";
import type { Recipe, SavedRecipeInfo } from "../state/api";
import type { T } from "../i18n";

/**
 * The saved-recipe library (SPEC.md FR-55).
 *
 * Saving is keyed on the recipe name, so saving twice under one name replaces rather
 * than accumulating near-identical entries. Loading restores every field, including
 * the layer exclusions, which is why the whole recipe travels rather than a summary.
 */
export function RecipeLibrary({
  t,
  recipe,
  onLoad,
}: {
  t: T;
  /** The current recipe, when there is one to save. */
  recipe: Recipe | null;
  onLoad: (r: Recipe) => void;
}) {
  const [saved, setSaved] = useState<SavedRecipeInfo[]>([]);
  const [name, setName] = useState("");
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setSaved(await api.listRecipes());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Seed the field from the recipe's generated name, but never overwrite typing.
  useEffect(() => {
    setName((n) => (n === "" && recipe ? recipe.name : n));
  }, [recipe]);

  const save = async () => {
    if (!recipe) return;
    setError(null);
    try {
      const id = await api.saveRecipe({ ...recipe, name: name.trim() || recipe.name });
      setNote(t("recipes.saved", { id }));
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const load = async (id: string) => {
    setError(null);
    try {
      const r = await api.loadRecipe(id);
      setName(r.name);
      onLoad(r);
    } catch (e) {
      setError(String(e));
    }
  };

  const remove = async (id: string) => {
    setError(null);
    try {
      await api.deleteRecipe(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  if (!recipe && saved.length === 0) return null;

  return (
    <details className="notes recipes" open={!recipe && saved.length > 0}>
      <summary>{t("recipes.title")}</summary>

      {recipe && (
        <div className="field">
          <label htmlFor="recipe-name">{t("recipes.name")}</label>
          <div className="row tight">
            <input
              id="recipe-name"
              value={name}
              placeholder={recipe.name}
              onChange={(e) => {
                setName(e.target.value);
                setNote(null);
              }}
            />
            <button type="button" onClick={() => void save()}>{t("recipes.save")}</button>
          </div>
        </div>
      )}

      {saved.length > 0 && (
        <ul className="saved-list">
          {saved.map((r) => (
            <li key={r.id}>
              <button type="button" className="saved" onClick={() => void load(r.id)}>
                <strong>{r.name}</strong>
                <span className="muted small">
                  {t(`preset.${r.preset}`)} · {r.areaLabel} · {r.areaKm2.toFixed(0)} km²
                </span>
                <span className="mono muted small">{r.deviceId}</span>
              </button>
              <button
                type="button"
                className="link"
                aria-label={t("recipes.delete")}
                onClick={() => void remove(r.id)}
              >
                {t("recipes.delete")}
              </button>
            </li>
          ))}
        </ul>
      )}

      {note && <p className="muted small">{note}</p>}
      {error && <p className="error small">{error}</p>}
    </details>
  );
}
