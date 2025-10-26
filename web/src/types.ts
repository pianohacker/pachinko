export const ITEM_SIZES = ["S", "M", "L", "X"];
export type ItemSize = (typeof ITEM_SIZES)[number];

export const prevSize = (size: ItemSize) =>
  ITEM_SIZES[
    ((ITEM_SIZES.findIndex((x) => x == size) || 0) - 1 + ITEM_SIZES.length) %
      ITEM_SIZES.length
  ];

export const nextSize = (size: ItemSize) =>
  ITEM_SIZES[
    ((ITEM_SIZES.findIndex((x) => x == size) || 0) + 1) % ITEM_SIZES.length
  ];

export type ItemLocation = {
  object_id: number | null;
  name: string;
  num_bins: number;
};

export type Item = {
  object_id: number | null;
  name: string;
  location: ItemLocation;
  bin_no: number;
  size: ItemSize;
};
