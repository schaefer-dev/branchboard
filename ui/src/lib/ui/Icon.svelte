<script lang="ts">
  const paths = {
    plus: "M8 3v10M3 8h10",
    chevron: "M6 4l4 4-4 4",
    star: "M8 2.5l1.7 3.5 3.8.5-2.8 2.6.7 3.8L8 11.1l-3.4 1.8.7-3.8L2.5 6.5l3.8-.5z",
    folder: "M2 4.5h4l1.5 1.5H14v6.5H2z",
    fetch: "M8 2.5v8M4.5 7 8 10.5 11.5 7M3 13.5h10",
    more: "M3.5 8h.01M8 8h.01M12.5 8h.01",
    external: "M9 3h4v4M13 3 7.5 8.5M11 9.5V13H3V5h3.5",
    comment: "M2.5 3h11v7.5h-6L4.5 13v-2.5h-2z",
    bell: "M4 11V7a4 4 0 0 1 8 0v4l1 1.5H3zM6.5 13.5h3",
    "bell-off": "M4 11V7a4 4 0 0 1 8 0v4l1 1.5H3zM6.5 13.5h3M2 2l12 12",
    eye: "M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8zM8 6.25a1.75 1.75 0 1 0 0 3.5 1.75 1.75 0 0 0 0-3.5z",
  } as const;

  interface Props {
    name: keyof typeof paths;
    size?: number;
    filled?: boolean;
  }

  let { name, size = 16, filled = false }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill={filled ? "currentColor" : "none"}
  stroke="currentColor"
  stroke-width="1.5"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={paths[name]} />
</svg>
