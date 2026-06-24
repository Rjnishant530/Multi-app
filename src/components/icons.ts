// Curated icon catalog rendered via lucide-react. The persisted
// Instance.icon value is the slug (e.g. "briefcase"); the lookup
// resolves it to a Lucide component at render time. Slugs not in the
// catalog (older data with literal emoji, custom text) render as the
// raw string via the fallback path in InstanceIcon.
//
// Every Lucide component is imported with an `XxxIcon` alias to avoid
// any chance of shadowing browser globals (Image, History, etc.) or
// being mangled by an unexpected scope collision. The catalog and
// picker both reference the aliased local names — slugs stay short.

import {
  Bookmark as BookmarkIcon,
  BookOpen as BookOpenIcon,
  Briefcase as BriefcaseIcon,
  Building2 as Building2Icon,
  Camera as CameraIcon,
  Coffee as CoffeeIcon,
  CreditCard as CreditCardIcon,
  DollarSign as DollarSignIcon,
  Flag as FlagIcon,
  Gamepad2 as Gamepad2Icon,
  GitBranch as GitBranchIcon,
  Globe as GlobeIcon,
  GraduationCap as GraduationCapIcon,
  Headphones as HeadphonesIcon,
  Heart as HeartIcon,
  Home as HomeIcon,
  Image as ImageIcon,
  Lock as LockIcon,
  Mail as MailIcon,
  MessageSquare as MessageSquareIcon,
  Music as MusicIcon,
  Palette as PaletteIcon,
  Phone as PhoneIcon,
  ShoppingCart as ShoppingCartIcon,
  Star as StarIcon,
  Store as StoreIcon,
  Tag as TagIcon,
  User as UserIcon,
  UserCircle as UserCircleIcon,
  Users as UsersIcon,
  Video as VideoIcon,
  Wallet as WalletIcon,
  type LucideIcon,
} from "lucide-react";

// Reserved icon used as the default fallback for child (forked)
// instances when the user hasn't picked a custom icon. Deliberately
// NOT in ICON_CATALOG / ICON_CHOICES so it can't be picked from the
// popup — it visually signals "this instance was forked from above".
export const ChildBranchIcon: LucideIcon = GitBranchIcon;

export const ICON_CATALOG: Record<string, LucideIcon> = {
  user: UserIcon,
  users: UsersIcon,
  "user-circle": UserCircleIcon,
  briefcase: BriefcaseIcon,
  building: Building2Icon,
  home: HomeIcon,
  store: StoreIcon,
  cart: ShoppingCartIcon,
  "credit-card": CreditCardIcon,
  dollar: DollarSignIcon,
  wallet: WalletIcon,
  heart: HeartIcon,
  star: StarIcon,
  bookmark: BookmarkIcon,
  flag: FlagIcon,
  tag: TagIcon,
  mail: MailIcon,
  message: MessageSquareIcon,
  phone: PhoneIcon,
  video: VideoIcon,
  camera: CameraIcon,
  image: ImageIcon,
  globe: GlobeIcon,
  lock: LockIcon,
  graduation: GraduationCapIcon,
  book: BookOpenIcon,
  palette: PaletteIcon,
  gamepad: Gamepad2Icon,
  music: MusicIcon,
  headphones: HeadphonesIcon,
  coffee: CoffeeIcon,
};

export interface IconChoice {
  slug: string;
  label: string;
  Icon: LucideIcon;
}

// Display order in the picker — keyed for visual grouping (people,
// commerce, social, media, misc).
export const ICON_CHOICES: IconChoice[] = [
  { slug: "user", label: "User", Icon: UserIcon },
  { slug: "users", label: "Users", Icon: UsersIcon },
  { slug: "user-circle", label: "Profile", Icon: UserCircleIcon },
  { slug: "briefcase", label: "Work", Icon: BriefcaseIcon },
  { slug: "building", label: "Company", Icon: Building2Icon },
  { slug: "home", label: "Personal", Icon: HomeIcon },
  { slug: "store", label: "Store", Icon: StoreIcon },
  { slug: "cart", label: "Shopping", Icon: ShoppingCartIcon },
  { slug: "credit-card", label: "Card", Icon: CreditCardIcon },
  { slug: "dollar", label: "Money", Icon: DollarSignIcon },
  { slug: "wallet", label: "Wallet", Icon: WalletIcon },
  { slug: "heart", label: "Heart", Icon: HeartIcon },
  { slug: "star", label: "Star", Icon: StarIcon },
  { slug: "bookmark", label: "Bookmark", Icon: BookmarkIcon },
  { slug: "flag", label: "Flag", Icon: FlagIcon },
  { slug: "tag", label: "Tag", Icon: TagIcon },
  { slug: "mail", label: "Mail", Icon: MailIcon },
  { slug: "message", label: "Chat", Icon: MessageSquareIcon },
  { slug: "phone", label: "Phone", Icon: PhoneIcon },
  { slug: "video", label: "Video", Icon: VideoIcon },
  { slug: "camera", label: "Camera", Icon: CameraIcon },
  { slug: "image", label: "Image", Icon: ImageIcon },
  { slug: "globe", label: "Web", Icon: GlobeIcon },
  { slug: "lock", label: "Lock", Icon: LockIcon },
  { slug: "graduation", label: "School", Icon: GraduationCapIcon },
  { slug: "book", label: "Book", Icon: BookOpenIcon },
  { slug: "palette", label: "Design", Icon: PaletteIcon },
  { slug: "gamepad", label: "Games", Icon: Gamepad2Icon },
  { slug: "music", label: "Music", Icon: MusicIcon },
  { slug: "headphones", label: "Audio", Icon: HeadphonesIcon },
  { slug: "coffee", label: "Coffee", Icon: CoffeeIcon },
];
