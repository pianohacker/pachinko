import { memo, Suspense, use, useDeferredValue, useMemo } from "react";
import { fetchApi } from "./api";
import type { Item } from "./types";

import itemClasses from "./ItemList.module.css";
import React from "react";
import { AsyncFzf, type FzfResultItem } from "fzf";

const Highlightable = ({
  start,
  className,
  positions,
  children,
}: {
  start: number;
  className: string;
  positions: Set<number>;
  children: string;
}) => {
  if (positions.size == 0) return <div className={className}>{children}</div>;

  return (
    <div
      className={className}
      dangerouslySetInnerHTML={{
        __html: Array.from(children)
          .map((c, i) => (positions.has(start + i) ? `<b>${c}</b>` : c))
          .join(""),
      }}
    />
  );

  // This algorithm is more clever but less performant overall. Turns out the browser is actually
  // pretty fast at rendering a bajillion one-character nodes.
  //
  //   const spans = [];

  //   let i = 0;
  //   while (i < children.length) {
  //     const hlStart = i;
  //     while (positions.has(start + i)) i++;
  //     if (i > hlStart)
  //       spans.push(`<strong>${children.substring(hlStart, i)}</strong>`);

  //     const noHlStart = i;
  //     while (i < children.length && !positions.has(start + i)) i++;
  //     if (i > noHlStart) spans.push(children.substring(noHlStart, i));
  //   }

  //   return (
  //     <div
  //       className={className}
  //       dangerouslySetInnerHTML={{ __html: spans.join("") }}
  //     />
  //   );
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

    const [fzfV1, fzfV2] = useMemo(
      () =>
        (["v1", "v2"] as ("v1" | "v2")[]).map(
          (fuzzy) =>
            new AsyncFzf(items, {
              fuzzy,
              tiebreakers: [
                (a, b) =>
                  a.item.location.name.localeCompare(b.item.location.name),
                (a, b) => a.item.bin_no - b.item.bin_no,
                (a, b) => a.item.name.localeCompare(b.item.name),
              ],
              selector(item) {
                return `${item.location.name}/${item.bin_no} ${item.name}`;
              },
            }),
        ),

      [items],
    );

    const allItemsPromise = useMemo(async () => {
      const allItems: FzfResultItem<Item>[] = items.map((item) => ({
        item,
        positions: new Set(),
        start: 0,
        end: 0,
        score: 0,
      }));
      allItems.sort(
        (a, b) =>
          a.item.location.name.localeCompare(b.item.location.name) ||
          a.item.bin_no - b.item.bin_no ||
          a.item.name.localeCompare(b.item.name),
      );
      console.log(allItems);
      return allItems;
    }, [items]);

    const filteredItemsPromise = useMemo(
      () =>
        search
          ? (search.length > 3 ? fzfV2 : fzfV1).find(search)
          : allItemsPromise,
      [fzfV1, fzfV2, allItemsPromise, search],
    );

    const filteredItems = use(filteredItemsPromise);

    if (filteredItems.length < 10) {
      console.log(filteredItems);
    }

    return (
      <div className={itemClasses.list}>
        {filteredItems.map(({ item, positions }) => {
          const binNoStartIndex = item.location.name.length + 1;
          const nameStartIndex =
            `${item.location.name}/${item.bin_no}`.length + 1;

          return (
            <React.Fragment key={item.object_id}>
              <Highlightable
                start={0}
                positions={positions}
                className={itemClasses.itemLocation}
              >
                {item.location.name}
              </Highlightable>
              <div className={itemClasses.itemSlash}>/</div>
              <Highlightable
                start={binNoStartIndex}
                positions={positions}
                className={itemClasses.itemBinNo}
              >
                {item.bin_no.toString()}
              </Highlightable>
              <Highlightable
                start={nameStartIndex}
                positions={positions}
                className={itemClasses.itemName}
              >
                {item.name}
              </Highlightable>
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
