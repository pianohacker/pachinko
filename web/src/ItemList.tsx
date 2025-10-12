import { Suspense, use, useDeferredValue, useMemo } from "react";
import { fetchApi } from "./api";
import type { Item } from "./types";

import itemClasses from "./ItemList.module.css";
import { LRUCache } from "lru-cache";
import React from "react";

const itemsCache = new LRUCache<string, Item[]>({
  max: 100,
  ttl: 1000 * 15,

  async fetchMethod(search, _staleValue, { signal }) {
    return fetchApi("/items?" + new URLSearchParams({ q: search }), {
      signal,
    }).then((r) => r.json()) as Promise<Item[]>;
  },
});

const ItemsListInner = ({
  itemsPromise,
}: {
  itemsPromise: Promise<Item[]>;
}) => {
  const items = use(itemsPromise);

  return (
    <div className={itemClasses.list}>
      {items.map((item) => (
        <React.Fragment key={item.object_id}>
          <div className={itemClasses.itemLocation}>{item.location.name}</div>
          <div className={itemClasses.itemSlash}>/</div>
          <div className={itemClasses.itemBinNo}>{item.bin_no}</div>
          <div className={itemClasses.itemName}>{item.name}</div>
        </React.Fragment>
      ))}
    </div>
  );
};

export const ItemList = ({ search }: { search: string }) => {
  const deferredSearch = useDeferredValue(search, "");

  const itemsPromise = useMemo(() => {
    const modifiedSearch = deferredSearch
      .split(/\s+/)
      .filter((word) => !!word)
      .map((word) => `*${word}*`)
      .join(" ");
    return itemsCache.fetch(modifiedSearch) as Promise<Item[]>;
  }, [deferredSearch]);

  return (
    <section className={itemClasses.listContainer}>
      <Suspense fallback={<p>Loading ...</p>}>
        <ItemsListInner itemsPromise={itemsPromise} />
      </Suspense>
    </section>
  );
};
