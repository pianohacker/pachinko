import { useCallback, useEffect, useRef, useState } from "react";
import addItemClasses from "./AddItem.module.css";
import {
  type ItemSize,
  type Item,
  type ItemLocation,
  nextSize,
  prevSize,
} from "./types";
import { fetchApi } from "./api";

export const AddItem = ({
  locations,
  lastItemAddTimestamp,
  isItemAdding,
  onAddItem,
}: {
  locations: ItemLocation[];
  lastItemAddTimestamp: Date | null;
  isItemAdding: boolean;
  onAddItem: (item: Item) => void;
}) => {
  const [hidden, setHidden] = useState(true);
  const [location, setLocation] = useState<ItemLocation>(locations[0]);
  const [binNo, setBinNo] = useState<number>(1);
  const [isBinNoLoading, setIsBinNoLoading] = useState(false);
  const [name, setName] = useState<string>("");
  const [size, setSize] = useState<ItemSize>("S");
  const nameInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setIsBinNoLoading(true);
    fetchApi(`/locations/${location.object_id!}/next-item-bin`)
      .then((r) => r.json())
      .then(({ bin_no }: { bin_no: number }) => {
        setBinNo(bin_no);
      })
      .finally(() => setIsBinNoLoading(false));
  }, [location, lastItemAddTimestamp]);

  useEffect(() => {
    if (!hidden) {
      setName("");
      nameInputRef.current?.focus();
    }
  }, [lastItemAddTimestamp, hidden]);

  const decreaseSize = useCallback(() => {
    setSize(prevSize);
  }, []);

  const increaseSize = useCallback(() => {
    setSize(nextSize);
  }, []);

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      switch (e.key) {
        case "Enter":
          e.preventDefault();

          if (isBinNoLoading || isItemAdding || !name) return;

          nameInputRef.current?.blur();
          onAddItem({
            object_id: null,
            location,
            bin_no: binNo,
            name,
            size,
          });
          break;

        case "ArrowUp":
          increaseSize();
          break;

        case "ArrowDown":
          decreaseSize();
          break;
      }
    },
    [isBinNoLoading, isItemAdding, location, binNo, name, size, onAddItem],
  );

  const onClickToggle = useCallback(() => {
    setHidden((hidden) => {
      if (hidden) {
        setName("");
        setTimeout(() => nameInputRef.current?.focus(), 150);
      }

      return !hidden;
    });
  }, []);

  return (
    <div className={addItemClasses.container}>
      <section
        className={
          addItemClasses.addItem + (hidden ? ` ${addItemClasses.hidden}` : "")
        }
      >
        <div
          className={
            addItemClasses.loadingOverlay +
            " " +
            (isItemAdding ? addItemClasses.visible : addItemClasses.hidden)
          }
        />
        <div className={addItemClasses.toggle} onClick={onClickToggle}>
          Add +
        </div>
        <div className={addItemClasses.inputs} onKeyDown={onKeyDown}>
          <select
            value={location.object_id!.toString()}
            onChange={(e) =>
              setLocation(
                locations.find(
                  ({ object_id }) => object_id!.toString() == e.target.value,
                )!,
              )
            }
          >
            {locations.map((location) => (
              <option
                key={location.object_id}
                value={location.object_id!.toString()}
              >
                {location.name}
              </option>
            ))}
          </select>

          <input
            className={addItemClasses.binNoInput}
            type="number"
            disabled={isBinNoLoading}
            value={binNo.toString()}
            onChange={(e) => {
              const binNo = parseInt(e.target.value);

              if (!isNaN(binNo)) setBinNo(binNo);
            }}
            min={1}
            max={location.num_bins}
          />

          <input
            ref={nameInputRef}
            className={addItemClasses.nameInput}
            value={name.toString()}
            onChange={(e) => setName(e.target.value)}
          />

          <button className={addItemClasses.sizePicker} onClick={increaseSize}>
            {size}
          </button>
        </div>
      </section>
    </div>
  );
};
