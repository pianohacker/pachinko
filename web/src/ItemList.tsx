import {
  memo,
  Suspense,
  use,
  useCallback,
  useDeferredValue,
  useMemo,
} from "react";
import { fetchApi } from "./api";
import type { Item } from "./types";

import itemClasses from "./ItemList.module.css";
import fuzzysort from "fuzzysort";
import {
  ModuleRegistry,
  ClientSideRowModelModule,
  ValidationModule,
  type ColDef,
  themeQuartz,
  CellStyleModule,
  TextEditorModule,
} from "ag-grid-community";
import { AgGridReact } from "ag-grid-react";

ModuleRegistry.registerModules([
  ClientSideRowModelModule,
  CellStyleModule,
  TextEditorModule,
]);
// via process.env.NODE_ENV
if (import.meta.env.VITE_ENV !== "production") {
  ModuleRegistry.registerModules([ValidationModule]);
}

type SearchResult = {
  item: Item;
  columns: string[];
  score: number;
  highlights: (() => string)[];
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

const agGridTheme = themeQuartz.withParams({
  borderWidth: 0,
  cellHorizontalPadding: ".25rem",
  fontSize: "1.5rem",
});

const cellRenderer =
  (i: number) =>
  ({ data }: { data: SearchResult }) => {
    const highlighted = data.highlights[i]?.();
    return highlighted ? (
      <span dangerouslySetInnerHTML={{ __html: highlighted }} />
    ) : (
      data.columns[i]
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
        highlights: [],
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
            result.obj.location.name,
            result.obj.bin_no.toString(),
            result.obj.name,
          ],
          highlights: [
            result[0].highlight.bind(result[0]),
            result[1].highlight.bind(result[1]),
            result[2].highlight.bind(result[2]),
          ],
        }));
      results.sort(compareResults);
      return results;
    }, [items, allResults, search]);

    if (filteredResults.length < 10) {
      console.log(filteredResults);
    }

    const colDefs: ColDef<SearchResult>[] = useMemo(
      () => [
        {
          colId: "location",
          cellClass: itemClasses.itemLocation,
          cellRenderer: cellRenderer(0),
          editable: true,
          cellEditor: "agTextCellEditor",
          valueGetter: ({ data }) => data?.columns[0],
        },
        {
          valueGetter: () => "/",
          width: 10,
          cellClass: itemClasses.itemSlash,
          selectable: false,
        },
        {
          colId: "bin_no",
          width: 40,
          cellClass: itemClasses.itemBinNo,
          cellRenderer: cellRenderer(1),
          editable: true,
          valueGetter: ({ data }) => data?.columns[1],
        },
        {
          colId: "name",
          flex: 1,
          cellRenderer: cellRenderer(2),
          editable: true,
          cellEditor: "agTextCellEditor",
          valueGetter: ({ data }) => data?.columns[2],
        },
      ],
      [],
    );

    const onCellEditRequest = useCallback((event) => {
      console.log({ event });
    }, []);

    return (
      <AgGridReact
        rowData={filteredResults}
        columnDefs={colDefs}
        theme={agGridTheme}
        animateRows={false}
        getRowId={(result) => (result.data.item.object_id || 0).toString()}
        readOnlyEdit
        onCellEditRequest={onCellEditRequest}
      />
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
