export type ItemSize = "S" | "M" | "L" | "X";

export type Location = {
  object_id: number | null;
  name: string;
  num_bins: number;
};

export type Item = {
  object_id: number | null;
  name: string;
  location: Location;
  bin_no: number;
  size: String;
};
