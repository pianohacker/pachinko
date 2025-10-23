export type ItemSize = "S" | "M" | "L" | "X";

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
  size: string;
};
