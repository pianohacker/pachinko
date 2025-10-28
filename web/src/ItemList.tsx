import {
  memo,
  Suspense,
  use,
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { fetchApi } from "./api";
import type { Item, ItemLocation } from "./types";

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
  SelectEditorModule,
  type CellEditingStoppedEvent,
  RowStyleModule,
  RenderApiModule,
  NumberEditorModule,
  type GetRowIdParams,
  ClientSideRowModelApiModule,
  ScrollApiModule,
  type RowClassRules,
  type CellRendererSelectorFunc,
} from "ag-grid-community";
import { AgGridReact } from "ag-grid-react";
import { AddItem } from "./AddItem";

ModuleRegistry.registerModules([
  CellStyleModule,
  ClientSideRowModelApiModule,
  ClientSideRowModelModule,
  NumberEditorModule,
  RenderApiModule,
  RowStyleModule,
  ScrollApiModule,
  SelectEditorModule,
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
  highlights: ((() => string) | undefined)[];
};

const compareResults = (a: SearchResult, b: SearchResult): number =>
  b.score - a.score ||
  a.item.location.name.localeCompare(b.item.location.name) ||
  a.item.bin_no - b.item.bin_no ||
  a.item.name.localeCompare(b.item.name);

const agGridTheme = themeQuartz.withParams({
  backgroundColor: "transparent",
  borderWidth: 0,
  cellHorizontalPadding: ".25rem",

  rowHoverColor: "var(--color-hover)",
  fontFamily: "Spectral",
  fontSize: "1rem",
});

const HighlightCellRenderer = ({
  i,
  data,
}: {
  i: number;
  data: SearchResult;
}) => {
  const highlighted = data.highlights[i]?.();

  return highlighted ? (
    <span dangerouslySetInnerHTML={{ __html: highlighted }} />
  ) : (
    data.columns[i]
  );
};

const cellRendererSelector =
  (i: number): CellRendererSelectorFunc<SearchResult> =>
  ({ data }) =>
    data?.highlights[i]
      ? {
          component: HighlightCellRenderer,
          params: { i },
        }
      : undefined;

const ItemsListInner = memo(
  ({
    itemsPromise,
    locationsPromise,
    search,
    setErrorMessage,
  }: {
    itemsPromise: Promise<Item[]>;
    locationsPromise: Promise<ItemLocation[]>;
    search: string;
    setErrorMessage: (x: string) => void;
  }) => {
    const items = use(itemsPromise);
    const locations = use(locationsPromise);

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

    const filteredResults = useMemo(() => {
      if (!search) return allResults;

      const results = fuzzysort
        .go(search, items, {
          keys: ["location.name", "bin_no", "name"],
        })
        .map(
          (result): SearchResult => ({
            item: result.obj,
            score: result.score,
            columns: [
              result.obj.location.name,
              result.obj.bin_no.toString(),
              result.obj.name,
            ],
            highlights: [
              result[0].indexes.length > 0
                ? result[0].highlight.bind(result[0])
                : undefined,
              result[1].indexes.length > 0
                ? result[1].highlight.bind(result[1])
                : undefined,
              result[2].indexes.length > 0
                ? result[2].highlight.bind(result[2])
                : undefined,
            ],
          }),
        );
      results.sort(compareResults);
      return results;
    }, [items, allResults, search]);

    const colDefs = useMemo(
      (): ColDef<SearchResult>[] => [
        {
          colId: "location",
          cellClass: itemClasses.itemLocation,
          cellRendererSelector: cellRendererSelector(0),
          flex: 1,
          editable: true,
          cellEditor: "agSelectCellEditor",
          cellEditorParams: {
            values: locations.map(({ name }) => name),
          },
          valueGetter: ({ data }) => data?.columns[0],
          valueSetter: ({ data, newValue }) => {
            const newLocation = locations.find(({ name }) => name == newValue);
            if (!newLocation) return false;

            delete data.highlights[0];
            data.item.location = newLocation;
            data.columns[0] = newValue;
            return true;
          },
        },
        {
          valueGetter: () => "/",
          width: 5,
          cellClass: itemClasses.itemSlash,
        },
        {
          colId: "bin_no",
          width: 40,
          cellClass: itemClasses.itemBinNo,
          cellRendererSelector: cellRendererSelector(1),
          editable: true,
          cellEditorSelector: ({ data }) => ({
            component: "agNumberCellEditor",
            params: {
              min: 1,
              max: data.item.location.num_bins,
            },
          }),
          valueGetter: ({ data }) => data?.columns[1],
          valueSetter: ({ data, newValue }) => {
            const newBinNo = parseInt(newValue);
            if (isNaN(newBinNo)) return false;

            delete data.highlights[1];
            data.item.bin_no = newBinNo;
            data.columns[1] = newBinNo.toString();
            return true;
          },
        },
        {
          colId: "name",
          flex: 4,
          cellClass: itemClasses.itemName,
          cellRendererSelector: cellRendererSelector(2),
          editable: true,
          cellEditor: "agTextCellEditor",
          valueGetter: ({ data }) => data?.columns[2],
          valueSetter: ({ data, newValue }) => {
            const newName = (newValue as string).trim();
            if (!newName) return false;

            delete data.highlights[2];
            data.item.name = newValue;
            data.columns[2] = newValue;
            return true;
          },
        },
        {
          colId: "size",
          width: 56,
          field: "item.size",
          cellClass: itemClasses.itemSize,
        },
      ],
      [locations],
    );

    const rowClassRules: RowClassRules = useMemo(
      () => ({
        [itemClasses.lowScore]: "data.score < 0.3",
      }),
      [],
    );

    type ItemUpdate = { location_id?: number } & Partial<
      Pick<Item, "bin_no" | "name" | "size">
    >;

    const dispatchUpdate = useCallback(
      async (object_id: number, update: ItemUpdate) => {
        try {
          await fetchApi(`/items/${object_id}`, {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
            },
            body: JSON.stringify(update),
          });
        } catch (e) {
          console.error(`Failed to update item: ${e}`);
          setErrorMessage(`Failed to update item, please reload: ${e}`);
        }
      },
      [setErrorMessage],
    );

    const onCellEditingStopped = useCallback(
      (event: CellEditingStoppedEvent<SearchResult>) => {
        if (!event.data || !event.valueChanged) return;
        const { item } = event.data;

        const update: ItemUpdate = {};

        switch (event.colDef.colId) {
          case "location":
            if (!item.location.object_id) return;

            update.location_id = item.location.object_id;
            break;
          case "bin_no":
            update.bin_no = item.bin_no;
            break;
          case "name":
            update.name = item.name;
            break;
        }

        dispatchUpdate(item.object_id!, update);
      },
      [dispatchUpdate],
    );

    const gridRef = useRef<AgGridReact<SearchResult> | null>(null);

    useEffect(() => {
      if (!gridRef.current?.api || gridRef.current.api.isDestroyed()) return;

      gridRef.current.api.refreshCells({ force: true });
      gridRef.current.api.ensureIndexVisible(0);
    }, [filteredResults]);

    const getRowId = useCallback(
      (result: GetRowIdParams<SearchResult>) =>
        (result.data.item.object_id || 0).toString() +
        (result.data.score < 0.3),
      [],
    );

    const [lastItemAddTimestamp, setLastItemAddTimestamp] =
      useState<Date | null>(null);

    const [isItemAdding, setIsItemAdding] = useState(false);

    const onAddItem = useCallback(
      async (item: Item) => {
        setIsItemAdding(true);

        let object_id: number;

        try {
          const response = await fetchApi(`/items`, {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              ...item,
              location_id: item.location.object_id,
            }),
          });

          ({ object_id } = (await response.json()) as { object_id: number });
        } catch (e) {
          console.error(`Failed to add item: ${e}`);
          setErrorMessage(`Failed to add item, please reload: ${e}`);

          return;
        } finally {
          setLastItemAddTimestamp(new Date());
          setIsItemAdding(false);
        }

        item.object_id = object_id;
        items.unshift(item);

        if (!gridRef.current?.api || gridRef.current.api.isDestroyed()) return;

        const newResult: SearchResult = {
          item,
          columns: [item.location.name, item.bin_no.toString(), item.name],
          score: 1,
          highlights: [],
        };

        gridRef.current.api.applyTransaction({
          addIndex: 0,
          add: [newResult],
        });

        gridRef.current.api.ensureIndexVisible(0);
      },
      [items, setErrorMessage],
    );

    return (
      <>
        <AgGridReact
          ref={gridRef}
          className={itemClasses.grid}
          theme={agGridTheme}
          rowData={filteredResults}
          rowClassRules={rowClassRules}
          getRowId={getRowId}
          animateRows={false}
          columnDefs={colDefs}
          singleClickEdit
          onCellEditingStopped={onCellEditingStopped}
        />
        <AddItem
          locations={locations}
          lastItemAddTimestamp={lastItemAddTimestamp}
          isItemAdding={isItemAdding}
          onAddItem={onAddItem}
        />
      </>
    );
  },
);

export const ItemList = ({ search }: { search: string }) => {
  const deferredSearch = useDeferredValue(search, "");

  const itemsPromise = useMemo(
    () => fetchApi("/items").then((r) => r.json()) as Promise<Item[]>,
    [],
  );

  const locationsPromise = useMemo(
    () =>
      fetchApi("/locations").then((r) => r.json()) as Promise<ItemLocation[]>,
    [],
  );

  const [errorMessage, setErrorMessage] = useState("");

  return (
    <section className={itemClasses.listContainer}>
      {errorMessage && (
        <div className={itemClasses.errorMessage}>{errorMessage}</div>
      )}
      <Suspense fallback={<p>Loading ...</p>}>
        <ItemsListInner
          itemsPromise={itemsPromise}
          locationsPromise={locationsPromise}
          search={deferredSearch}
          setErrorMessage={setErrorMessage}
        />
      </Suspense>
    </section>
  );
};
