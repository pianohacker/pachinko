import { memo, Suspense, use, useDeferredValue, useMemo } from "react";
import { fetchApi } from "./api";
import type { Item } from "./types";

import itemClasses from "./ItemList.module.css";
import React from "react";
import fuzzysort from "fuzzysort";

type SearchResult = {
  item: Item;
  columns: string[];
  score: number;
};

const compareResults = (a: SearchResult, b: SearchResult): number =>
  Math.round(b.score * 20) / 20 - Math.round(a.score * 20) / 20 ||
  a.item.location.name.localeCompare(b.item.location.name) ||
  a.item.bin_no - b.item.bin_no ||
  a.item.name.localeCompare(b.item.name);

const Highlightable = ({
  result,
  column,
  className,
}: {
  result: SearchResult;
  column: number;
  className: string;
}) => {
  return (
    <div
      className={
        className + (result.score < 0.4 ? ` ${itemClasses.lowScore}` : "")
      }
      dangerouslySetInnerHTML={{
        __html: result.columns[column],
      }}
    />
  );
};

const ItemsListInner = memo(
  ({
    itemsPromise,
    search,
  }: {
    itemsPromise: Promise<Item[]>;
    search: string;
  }) => {
    const items = use(itemsPromise);

    const allResults: SearchResult[] = useMemo(() => {
      const results = items.map((item) => ({
        item,
        score: 1,
        columns: [item.location.name, item.bin_no.toString(), item.name],
      }));
      results.sort(compareResults);
      return results;
    }, [items]);

    const filteredResults: SearchResult[] = useMemo(() => {
      if (!search) return allResults;

      const results = fuzzysort
        .go(search, items, {
          keys: ["location.name", "bin_no", "name"],
        })
        .map((result) => ({
          item: result.obj,
          score: result.score,
          columns: [
            result[0].score ? result[0].highlight() : result.obj.location.name,
            result[1].score
              ? result[1].highlight()
              : result.obj.bin_no.toString(),
            result[2].score ? result[2].highlight() : result.obj.name,
          ],
        }));
      results.sort(compareResults);
      return results;
    }, [items, allResults, search]);

    if (filteredResults.length < 10) {
      console.log(filteredResults);
    }

    return (
      <div className={itemClasses.list}>
        {filteredResults.map((result) => {
          const item = result.item;

          return (
            <React.Fragment key={item.object_id}>
              <Highlightable
                result={result}
                column={0}
                className={itemClasses.itemLocation}
              />
              <div className={itemClasses.itemSlash}>/</div>
              <Highlightable
                result={result}
                column={1}
                className={itemClasses.itemBinNo}
              />
              <Highlightable
                result={result}
                column={2}
                className={itemClasses.itemName}
              />
            </React.Fragment>
          );
        })}
      </div>
    );
  },
);

export const ItemList = ({ search }: { search: string }) => {
  const deferredSearch = useDeferredValue(search, "");

  const itemsPromise = useMemo(
    () => fetchApi("/items").then((r) => r.json()) as Promise<Item[]>,
    [],
  );

  return (
    <section className={itemClasses.listContainer}>
      <Suspense fallback={<p>Loading ...</p>}>
        <ItemsListInner itemsPromise={itemsPromise} search={deferredSearch} />
      </Suspense>
    </section>
  );
};
