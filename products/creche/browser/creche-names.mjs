const encoder = new TextEncoder();
const MAX_FRIENDLY_NAME_BYTES = 64;

// Recovered from basic-os a4ac9d3's UUID-indexed naming sketch. This catalog
// keeps the useful deterministic-persona idea, but does not claim reversibility:
// these suggestions are deliberately collision-tolerant metadata.
// Suggestions are friendly metadata, not claims that a culture has one universal
// naming rule. Components stay inside their named structure and are never mixed
// across systems; the fantasy material below is original and labeled as inspired.

const romanPraenomina = ["Aulus", "Appius", "Decimus", "Gaius", "Gnaeus", "Kaeso", "Lucius", "Manius", "Marcus", "Numerius", "Publius", "Quintus", "Servius", "Sextus", "Spurius", "Tiberius", "Titus", "Vibius"];
const romanNomina = ["Aelius", "Aemilius", "Antonius", "Appius", "Atilius", "Aurelius", "Caecilius", "Cassius", "Claudius", "Cornelius", "Domitius", "Fabius", "Flavius", "Fulvius", "Junius", "Licinius", "Livius", "Lucretius", "Marcius", "Octavius", "Pompeius", "Sergius", "Valerius", "Vergilius"];
const romanCognomina = ["Agricola", "Aquila", "Cato", "Celer", "Cicero", "Corvinus", "Felix", "Flaccus", "Florus", "Gallus", "Laetus", "Lentulus", "Lepidus", "Longinus", "Marcellus", "Maximus", "Naso", "Paullus", "Priscus", "Rufus", "Sabinus", "Severus", "Varro", "Vetus"];

const chineseFamilyNames = ["Wáng", "Li", "Zhang", "Liu", "Chen", "Yang", "Huang", "Zhao", "Wu", "Zhou", "Xu", "Sun", "Ma", "Zhu", "Hu", "Guo", "He", "Gao", "Lin", "Luo", "Zheng", "Liang", "Xie", "Song", "Tang", "Xǔ", "Han", "Feng", "Deng", "Cao", "Peng", "Zeng", "Xiao", "Tian", "Dong", "Pan", "Yuan", "Cai", "Jiǎng", "Yú", "Yu", "Du", "Ye", "Cheng", "Su", "Wei", "Lü", "Ding", "Ren", "Shen", "Yao", "Lu", "Jiāng", "Cui", "Zhong", "Tan", "Lù", "Wāng", "Fan", "Jin", "Shi", "Liao", "Jia", "Xia"];
const chineseGivenNames = ["Anran", "Bowen", "Chenxi", "Chunhua", "Haoran", "Heguang", "Jianing", "Jingming", "Jingyi", "Junjie", "Kexin", "Lecheng", "Mingda", "Minghui", "Ningyuan", "Ruoshui", "Siyuan", "Tianyou", "Wenxuan", "Xinran", "Xinghe", "Xiuying", "Yajing", "Yanbo", "Yangchun", "Yifan", "Yiyun", "Yongning", "Yutong", "Yulan", "Yuexin", "Zeyu", "Zihan", "Zixuan", "Shihan", "Yuhang", "Jiayi", "Jia Ning", "Junxi", "Mingyu", "Ruixue", "Siqi", "Siyu", "Tianle", "Wenbo", "Xiaotong", "Xinyi", "Xingchen", "Yawen", "Yichen", "Yuchen", "Yuxin", "Yu Tong", "Yujie", "Yuehua", "Yunfan", "Zelin", "Zhiyuan", "Zimo", "Zi Xuan", "Zhuoran", "Qingyang", "Huaijin", "Zhixia"];

const americanGivenNames = ["Avery", "Bailey", "Blake", "Cameron", "Casey", "Charlie", "Dakota", "Drew", "Eden", "Elliot", "Emerson", "Finley", "Frankie", "Harper", "Hayden", "Jamie", "Jordan", "Kai", "Kendall", "Lane", "Logan", "Marley", "Morgan", "Noel", "Parker", "Payton", "Quinn", "Reese", "Remy", "Riley", "River", "Robin", "Rowan", "Sage", "Sam", "Shawn", "Skyler", "Taylor", "Terry", "Val", "Winter", "Wren", "Alex", "Ari", "Devin", "Jesse", "Micah", "Sidney", "Addison", "Adrian", "Ainsley", "Ashton", "Blair", "Brooklyn", "Campbell", "Carson", "Ellis", "Gray", "Jules", "Kennedy", "Lennon", "Phoenix", "Rory", "Spencer"];
const americanMiddleNames = ["A.", "B.", "C.", "D.", "E.", "F.", "G.", "H.", "J.", "K.", "L.", "M.", "N.", "P.", "Q.", "R.", "S.", "T.", "V.", "W.", "Blue", "Gray", "June", "Lee", "Ray", "Sky", "True", "Dawn", "Jean", "Mae", "Rose", "Wilde"];
const americanFamilyNames = ["Adams", "Allen", "Bailey", "Baker", "Bell", "Bennett", "Brooks", "Brown", "Butler", "Campbell", "Carter", "Clark", "Collins", "Cook", "Cooper", "Cox", "Davis", "Diaz", "Edwards", "Evans", "Flores", "Foster", "Garcia", "Gomez", "Gray", "Green", "Hall", "Harris", "Hayes", "Henderson", "Hill", "Howard", "Hughes", "Jackson", "James", "Jenkins", "Johnson", "Kelly", "King", "Lee", "Lewis", "Long", "Martin", "Martinez", "Miller", "Mitchell", "Moore", "Morgan", "Morris", "Murphy", "Nelson", "Ortiz", "Parker", "Patel", "Perry", "Price", "Ramirez", "Reed", "Rivera", "Roberts", "Ross", "Scott", "Stewart", "Turner", "Walker", "Ward", "Watson", "White", "Williams", "Wilson", "Wood", "Young"];

const mexicanMasculineGivenNames = ["Adrián", "Alejandro", "Ángel", "Carlos", "César", "Daniel", "Diego", "Emiliano", "Esteban", "Gabriel", "Gael", "Javier", "Joaquín", "José", "Leonardo", "Luis", "Manuel", "Mateo", "Nicolás", "Rafael", "Santiago"];
const mexicanFeminineGivenNames = ["Abril", "Alejandra", "Alma", "Camila", "Carmen", "Catalina", "Citlali", "Daniela", "Elena", "Emilia", "Fernanda", "Inés", "Irene", "Jimena", "Julia", "Lucía", "Luna", "Mar", "Mariana", "Natalia", "Noemí", "Paola", "Regina", "Renata", "Sofía", "Valentina", "Ximena"];
const mexicanGivenNames = [...mexicanMasculineGivenNames, ...mexicanFeminineGivenNames];
const mexicanMasculineAdditionalNames = ["Alejandro", "Antonio", "Daniel", "Emilio", "Enrique", "Felipe", "Gabriel", "Ignacio", "Jesús", "Joaquín", "José", "Juan", "Miguel", "Raúl", "Vicente"];
const mexicanFeminineAdditionalNames = ["Alejandra", "Beatriz", "Belén", "Elena", "Estela", "Guadalupe", "Isabel", "Laura", "María", "Paz", "Rosa", "Sofía", "Teresa", "Valeria", "Yolanda"];
const mexicanFamilyNames = ["Aguilar", "Álvarez", "Ávila", "Bautista", "Cabrera", "Campos", "Cárdenas", "Carrillo", "Castillo", "Castro", "Cervantes", "Chávez", "Contreras", "Cortés", "Cruz", "Delgado", "Domínguez", "Durán", "Escobar", "Espinoza", "Flores", "Fuentes", "Gallegos", "García", "Gómez", "González", "Guerrero", "Gutiérrez", "Hernández", "Herrera", "Ibarra", "Jiménez", "Lara", "López", "Luna", "Martínez", "Medina", "Méndez", "Mendoza", "Miranda", "Molina", "Montes", "Morales", "Moreno", "Muñoz", "Navarro", "Núñez", "Ochoa", "Ortega", "Ortiz", "Pacheco", "Paredes", "Peña", "Pérez", "Ramírez", "Ramos", "Reyes", "Ríos", "Rivera", "Rodríguez", "Rojas", "Romero", "Ruiz", "Salazar", "Sánchez", "Sandoval", "Santos", "Silva", "Solís", "Soto", "Torres", "Valdez", "Vargas", "Vázquez", "Vega", "Velázquez", "Zamora", "Zúñiga"];

const icelandicGivenNames = ["Alda", "Andri", "Arna", "Ásta", "Baldur", "Bjarni", "Dagur", "Edda", "Einar", "Elín", "Embla", "Freyja", "Gísli", "Guðrún", "Halla", "Hrafn", "Inga", "Íris", "Jökull", "Katla", "Lilja", "Logi", "Magnús", "Nanna", "Ólafur", "Ragna", "Rósa", "Sævar", "Snæfríður", "Sól", "Tinna", "Una", "Vaka", "Vigdís", "Yrsa", "Þór", "Árni", "Björk", "Brynja", "Dagný", "Davíð", "Eiður", "Eir", "Erla", "Finnur", "Gréta", "Gunnar", "Harpa", "Helga", "Hildur", "Hulda", "Jón", "Kári", "Kristín", "Lára", "Margrét", "Ragnar", "Sigríður", "Sindri", "Sólveig", "Steinar", "Svava", "Tómas", "Þóra"];
const icelandicParentGenitives = ["Árna", "Baldurs", "Björns", "Dags", "Davíðs", "Eiðs", "Einars", "Elínar", "Emblu", "Freyju", "Gísla", "Guðrúnar", "Gunnars", "Höllu", "Hrafns", "Ingu", "Jóns", "Kötlu", "Lilju", "Loga", "Magnúsar", "Nönnu", "Ólafs", "Ragnars", "Rósu", "Sævars", "Sigríðar", "Sólveigar", "Tinnu", "Vigdísar", "Yrsu", "Þórs", "Aldu", "Andra", "Ástu", "Bjarna", "Brynju", "Dagnýjar", "Eddu", "Eirar", "Erlu", "Finns", "Grétu", "Hörpu", "Helgu", "Hildar", "Huldu", "Jökuls", "Kára", "Kristínar", "Láru", "Margrétar", "Rögnu", "Sindra", "Steinars", "Svövu", "Tómasar", "Þóru"];
const icelandicEndings = ["son", "dóttir", "bur"];

const japaneseFamilyNames = ["Sato", "Suzuki", "Takahashi", "Tanaka", "Ito", "Watanabe", "Yamamoto", "Nakamura", "Kobayashi", "Kato", "Yoshida", "Yamada", "Sasaki", "Yamaguchi", "Matsumoto", "Inoue", "Kimura", "Hayashi", "Saitō", "Shimizu", "Yamazaki", "Mori", "Ikeda", "Hashimoto", "Abe", "Ishikawa", "Yamashita", "Nakajima", "Ishii", "Ogawa", "Maeda", "Okada", "Hasegawa", "Fujita", "Goto", "Kondo", "Murakami", "Endo", "Aoki", "Sakamoto", "Saito", "Fukuda", "Ota", "Nishimura", "Fujii", "Kaneko", "Okamoto", "Fujiwara", "Miura", "Nakagawa", "Nakano", "Harada", "Matsuda", "Takeuchi", "Ono", "Tamura", "Nakayama", "Wada", "Ishida", "Ueda", "Morita", "Hara", "Shibata", "Sakai"];
const japaneseGivenNames = ["Aiko", "Akari", "Akira", "Aoi", "Asahi", "Ayaka", "Chihiro", "Daichi", "Daiki", "Emi", "Haru", "Haruka", "Hayato", "Hikaru", "Hinata", "Hiro", "Honoka", "Ibuki", "Ichika", "Itsuki", "Izumi", "Kaede", "Kaito", "Kanata", "Kaori", "Kazuki", "Kei", "Ken", "Kenta", "Kōki", "Madoka", "Makoto", "Mei", "Minato", "Minori", "Misaki", "Mizuki", "Nagi", "Nagisa", "Nanami", "Nao", "Naoki", "Rei", "Ren", "Rena", "Riku", "Rin", "Ryō", "Saki", "Sakura", "Shō", "Shōta", "Sora", "Sōta", "Subaru", "Takumi", "Tsubasa", "Wakana", "Yui", "Yū", "Yūki", "Yūna", "Yūri", "Yūta"];

const arabicGivenLineages = ["Adil ibn", "Amal bint", "Amin ibn", "Anwar ibn", "Badr ibn", "Basma bint", "Dalia bint", "Farah bint", "Hadi ibn", "Hala bint", "Iman bint", "Jamal ibn", "Karim ibn", "Layla bint", "Lina bint", "Maha bint", "Malik ibn", "Mariam bint", "Mazin ibn", "Nadia bint", "Nadir ibn", "Nasim ibn", "Noor bint", "Omar ibn", "Rami ibn", "Rana bint", "Reem bint", "Salim ibn", "Samir ibn", "Sana bint", "Tariq ibn", "Yasmin bint"];
const arabicParentNames = ["Abbas", "Adnan", "Ahmad", "Ali", "Amin", "Anwar", "Faris", "Hadi", "Hamid", "Hasan", "Ibrahim", "Ismail", "Jamal", "Karim", "Khalil", "Mahmud", "Malik", "Mansur", "Nadir", "Nasir", "Omar", "Rashid", "Salim", "Samir"];
const arabicFamilyNames = ["al-Amin", "al-Badri", "al-Faruqi", "al-Hakim", "al-Hariri", "al-Hassan", "al-Jabiri", "al-Karim", "al-Khatib", "al-Masri", "al-Najjar", "al-Nouri", "al-Qasim", "al-Rashid", "al-Sabah", "al-Salim", "al-Shami", "al-Tahir", "al-Zahra", "Darwish", "Fahmi", "Hamdan", "Hanna", "Jaber", "Khalil", "Mansour", "Nassar", "Rahman", "Saad", "Saleh", "Younes", "Zayed"];

const frenchGivenNames = ["Alix", "Anaïs", "Andréa", "Baptiste", "Camille", "Céleste", "Claude", "Clément", "Dominique", "Élodie", "Émile", "Estelle", "Florent", "Gabriel", "Inès", "Jules", "Léa", "Léon", "Lou", "Maël", "Manon", "Margot", "Mathis", "Noémie", "Océane", "Rémi", "Romane", "Sacha", "Solène", "Théo", "Valentin", "Zoé", "Adrien", "Amandine", "Aurélien", "Bastien", "Capucine", "Chloé", "Corentin", "Élise", "Étienne", "Fabien", "Hugo", "Juliette", "Laurent", "Lison", "Loïc", "Lucie", "Malo", "Marceau", "Marion", "Mélanie", "Nathan", "Nina", "Olivier", "Pauline", "Raphaël", "Rosalie", "Séraphine", "Sylvain", "Tristan", "Victoire", "Yann", "Yasmine"];
const frenchCompoundGivenNames = ["Anne-Laure", "Charles-Henri", "Émilie-Rose", "François-Xavier", "Jean-Baptiste", "Jean-Luc", "Jean-Michel", "Jean-Noël", "Louis-Marie", "Marc-Antoine", "Marie-Amélie", "Marie-Claire", "Marie-Laure", "Marie-Lou", "Marie-Pierre", "Paul-André", "Anne-Claire", "Charles-Édouard", "Claire-Marie", "Jean-Charles", "Jean-François", "Jean-Louis", "Jean-Marie", "Louis-Philippe", "Marie-Anne", "Marie-France", "Marie-Hélène", "Marie-Noëlle", "Paul-Henri", "Pierre-Louis", "Rose-Marie", "Yves-Marie"];
const frenchFamilyNames = ["André", "Aubert", "Barbier", "Benoît", "Berger", "Bernard", "Blanc", "Bonnet", "Boucher", "Boyer", "Brun", "Chevalier", "Clément", "Colin", "David", "Denis", "Dubois", "Dufour", "Dumont", "Dupont", "Durand", "Fabre", "Faure", "Fournier", "François", "Garnier", "Gauthier", "Girard", "Guérin", "Henry", "Lacroix", "Lambert", "Laurent", "Leclerc", "Lefèvre", "Lemoine", "Leroy", "Marchand", "Martin", "Masson", "Mercier", "Michel", "Moreau", "Moulin", "Noël", "Perrin", "Petit", "Renard", "Richard", "Rivière", "Robert", "Rousseau", "Roux", "Simon", "Thomas", "Vincent"];

const britishMasculineGivenNames = ["Arthur", "Callum", "Ciaran", "Dylan", "Euan", "Freddie", "George", "Henry", "Jasper", "Lewis", "Oliver", "Oscar", "Rhys", "Theo", "Thomas", "William", "Alfie", "Angus", "Archie", "Edward", "Finlay", "Hamish", "Hugh", "Jack", "Owain", "Rupert", "Toby", "Wilfred"];
const britishFeminineGivenNames = ["Alice", "Amelia", "Beatrice", "Charlotte", "Eleanor", "Evelyn", "Florence", "Harriet", "Imogen", "Isla", "Lowri", "Maisie", "Nia", "Orla", "Poppy", "Saoirse", "Ailsa", "Bethan", "Bronwen", "Carys", "Daisy", "Edith", "Elspeth", "Freya", "Holly", "Iona", "Kitty", "Megan", "Molly", "Phoebe", "Seren"];
const britishNeutralGivenNames = ["Alex", "Ellis", "Morgan", "Rowan", "Rory"];
const britishGivenNames = [...britishMasculineGivenNames, ...britishFeminineGivenNames, ...britishNeutralGivenNames];
const britishMasculineMiddleNames = ["Arthur", "David", "Edward", "Henry", "James", "John", "Michael", "Thomas", "Charles", "George", "Joseph", "Robert", "William"];
const britishFeminineMiddleNames = ["Anne", "Catherine", "Claire", "Elizabeth", "Eve", "Frances", "Grace", "Jane", "Louise", "Mae", "May", "Rose", "Alice", "Edith", "Helen", "Margaret", "Mary", "Ruth", "Victoria"];
const britishFamilyNames = ["Armstrong", "Atkinson", "Baker", "Bell", "Bennett", "Campbell", "Carter", "Clarke", "Collins", "Cook", "Cooper", "Davies", "Dawson", "Edwards", "Evans", "Fletcher", "Foster", "Fraser", "Graham", "Grant", "Green", "Griffiths", "Hall", "Hamilton", "Harris", "Hughes", "Jackson", "Jenkins", "Jones", "Kelly", "Lewis", "Lloyd", "Macdonald", "Marshall", "Martin", "Mason", "Mitchell", "Morgan", "Morris", "Murray", "Owen", "Palmer", "Parker", "Patel", "Phillips", "Price", "Rees", "Roberts", "Robertson", "Scott", "Shaw", "Smith", "Stewart", "Taylor", "Thomas", "Thompson", "Turner", "Walker", "Ward", "Watson", "White", "Williams", "Wilson", "Wood", "Wright", "Young"];

const classicAnglophoneMasculineGivenNames = ["Bill", "Bob", "Charles", "David", "Edward", "Frank", "George", "James", "John", "Joseph", "Michael", "Paul", "Peter", "Richard", "Robert", "Ronald", "Stephen", "Thomas", "William", "Albert", "Alfred", "Arthur", "Brian", "Daniel", "Douglas", "Gary", "Harold", "Kenneth", "Larry", "Raymond", "Walter"];
const classicAnglophoneFeminineGivenNames = ["Alice", "Anne", "Barbara", "Betty", "Carol", "Deborah", "Diane", "Dorothy", "Elizabeth", "Helen", "Jane", "Janet", "Jean", "Linda", "Margaret", "Mary", "Nancy", "Patricia", "Ruth", "Sandra", "Sharon", "Susan", "Beverly", "Brenda", "Catherine", "Cheryl", "Donna", "Gloria", "Joan", "Judith", "Karen", "Pamela", "Shirley"];
const classicAnglophoneGivenNames = [...classicAnglophoneMasculineGivenNames, ...classicAnglophoneFeminineGivenNames];
const classicAnglophoneMasculineMiddleNames = ["David", "Edward", "James", "John", "Lee", "Michael", "Ray", "Thomas", "Allen", "Arthur", "George", "Joseph", "Wayne"];
const classicAnglophoneFeminineMiddleNames = ["Ann", "Anne", "Elizabeth", "Grace", "Jane", "Jean", "Lee", "Louise", "Mae", "Marie", "Rose", "Catherine", "Claire", "Dawn", "Elaine", "Frances", "Irene", "Ruth", "Sue"];
const classicAnglophoneFamilyNames = ["Anderson", "Baker", "Brown", "Campbell", "Carter", "Clark", "Collins", "Cooper", "Davis", "Edwards", "Evans", "Foster", "Green", "Hall", "Harris", "Hill", "Jackson", "Johnson", "Jones", "Kelly", "King", "Lewis", "Martin", "Miller", "Mitchell", "Moore", "Morgan", "Morris", "Murphy", "Nelson", "Parker", "Perry", "Reed", "Roberts", "Robinson", "Ross", "Scott", "Smith", "Stewart", "Taylor", "Thomas", "Thompson", "Turner", "Walker", "Ward", "Watson", "White", "Williams", "Wilson", "Wood", "Wright", "Young", "Adams", "Allen", "Bailey", "Bell", "Bennett", "Brooks", "Butler", "Cook", "Cox", "Fisher", "Gray", "Griffin", "Hayes", "Henderson", "Howard", "Hughes", "Jenkins", "Long", "Marshall", "Mason", "Price", "Russell", "Simmons", "Stone", "Warren", "Webb", "West", "Wallace"];

const koreanFamilyNames = ["Kim", "Lee", "Park", "Choi", "Jung", "Kang", "Cho", "Yoon", "Jang", "Lim", "Han", "Oh", "Seo", "Shin", "Kwon", "Hwang", "Ahn", "Song", "Jeon", "Hong", "Yoo", "Ko", "Moon", "Yang", "Son", "Bae", "Baek", "Heo", "Nam", "Shim", "Roh", "Ha"];
const koreanGivenNames = ["Ga-ram", "Ga-on", "Geon-u", "Na-rae", "Da-on", "Do-yun", "Min-seo", "Min-jun", "Bo-ram", "Seo-yeon", "Seo-yun", "Seo-jun", "So-yeon", "Su-bin", "Su-hyeon", "Si-u", "A-reum", "Ye-rin", "Ye-jun", "Yu-na", "Eun-u", "I-deun", "Ji-min", "Ji-u", "Ji-yun", "Ji-ho", "Chae-won", "Ha-neul", "Ha-yun", "Ha-jun", "Hyeon-u", "Hye-jin", "Ga-eun", "Geon-hui", "Gyeong-min", "Na-yeon", "Da-bin", "Do-hyeon", "Dong-hyeon", "Min-jae", "Bo-min", "Seo-hyeon", "Seon-u", "Seong-min", "So-hui", "Su-min", "Seung-hyeon", "A-in", "Ye-eun", "Ye-ji", "Yu-jin", "Yun-seo", "Jeong-min", "Ju-won", "Jun-seo", "Ji-a", "Ji-an", "Ji-hu", "Chae-yun", "Tae-min", "Ha-ram", "Hyeon-seo", "Hyeon-ji", "Hui-won"];

const vietnameseFamilyNames = ["Bùi", "Đặng", "Đinh", "Đỗ", "Dương", "Hồ", "Hoàng", "Huỳnh", "Lê", "Lý", "Mai", "Ngô", "Nguyễn", "Phạm", "Phan", "Tạ", "Trần", "Trịnh", "Võ", "Vũ", "Cao", "Châu", "Chu", "Đoàn", "Hà", "Kiều", "Lâm", "Lương", "Phùng", "Quách", "Tôn", "Vương"];
const vietnameseMiddleNames = ["An", "Anh", "Bảo", "Công", "Đức", "Gia", "Hải", "Hoài", "Hồng", "Hữu", "Khánh", "Minh", "Ngọc", "Quang", "Thanh", "Thuỳ", "Ánh", "Đình", "Hoàng", "Kim", "Mai", "Mạnh", "Nhật", "Phương", "Quốc", "Tấn", "Thành", "Thị", "Trọng", "Tuấn", "Văn", "Xuân"];
const vietnameseGivenNames = ["An", "Anh", "Bình", "Châu", "Chi", "Dũng", "Giang", "Hà", "Hải", "Hiếu", "Hương", "Khánh", "Lan", "Linh", "Long", "Mai", "Minh", "Nam", "Ngân", "Ngọc", "Nhung", "Phúc", "Quân", "Quỳnh", "Sơn", "Thảo", "Trang", "Trúc", "Tú", "Uyên", "Việt", "Xuân", "Ánh", "Bách", "Bảo", "Cường", "Diệp", "Đạt", "Đông", "Hạnh", "Hoa", "Hoàng", "Huy", "Khoa", "Kiên", "Lâm", "Lộc", "Nga", "Nhi", "Phong", "Phương", "Quang", "Quốc", "Tâm", "Thành", "Thiên", "Thu", "Thư", "Tiến", "Trí", "Trung", "Tuấn", "Vân", "Yến"];

const yorubaPersonalNames = ["Adéọlá", "Adéwálé", "Ayọ̀", "Bísí", "Bọ́lá", "Dámilọ́lá", "Ẹniọlá", "Fúnmi", "Ifẹ́", "Kẹ́hìndé", "Morẹ́nikẹ́", "Ọlámidé", "Olúwadámilọ́lá", "Táíwò", "Títílọ́pẹ́", "Tólú", "Yetúndé", "Abíọ́lá", "Adérónkẹ́", "Ayọ̀délé", "Bábátúndé", "Dáyọ̀", "Dúpẹ́", "Fọláṣadé", "Ìdòwú", "Kọ́láwọlé", "Mọ́pẹ́lọ́lá", "Ọlábísí", "Ọlúwafẹ́mi", "Rótìmí", "Sẹ́gun", "Yẹ́mí"];
const yorubaFamilyNames = ["Adébáyọ̀", "Adégòkè", "Adéyẹmí", "Àjàyí", "Akínyẹmí", "Babalọ́lá", "Bánkọ́lé", "Dáramọ́lá", "Fálọlá", "Fáṣínà", "Ọládìpọ̀", "Ọláníyàn", "Ọlátúnjí", "Ọní", "Oyèlówó", "Ṣóníbalẹ̀", "Adéjùmọ̀", "Adékúnlé", "Adéṣínà", "Akíntọ́lá", "Awólọ́wọ̀", "Bádéjọ", "Fábùnmi", "Fádípẹ̀", "Fáníyì", "Fátóyè", "Ọlátúnbọ̀sún", "Ọlúwole", "Ọmọ́táyọ̀", "Onífádé", "Oyediran", "Ṣóyínká"];

// Ukrainian spellings follow the Cabinet of Ministers' official Latin-alphabet
// transliteration table (Resolution No. 55), rather than Russian intermediates.
const ukrainianGivenNames = ["Oleksandr", "Andrii", "Artem", "Bohdan", "Danylo", "Denys", "Dmytro", "Ivan", "Kyrylo", "Maksym", "Marko", "Matvii", "Mykhailo", "Nazar", "Mykyta", "Oleksii", "Ostap", "Pavlo", "Petro", "Roman", "Serhii", "Taras", "Tymofii", "Vadym", "Viktor", "Vitalii", "Volodymyr", "Yaroslav", "Yevhen", "Yurii", "Zakhar", "Ihor", "Alina", "Anastasiia", "Anna", "Bohdana", "Daryna", "Diana", "Iryna", "Kateryna", "Khrystyna", "Larysa", "Lesia", "Liliia", "Liubov", "Mariia", "Maryna", "Marta", "Nadiia", "Nataliia", "Oksana", "Olena", "Oleksandra", "Polina", "Roksolana", "Sofiia", "Solomiia", "Svitlana", "Tamara", "Tetiana", "Valentyna", "Viktoriia", "Yana", "Yuliia"];
const ukrainianFamilyNames = ["Shevchenko", "Kovalenko", "Bondarenko", "Tkachenko", "Kovalchuk", "Kravchuk", "Polishchuk", "Boiko", "Melnyk", "Oliinyk", "Lysenko", "Pavlenko", "Petrenko", "Savchenko", "Marchenko", "Moroz", "Levchenko", "Rudenko", "Honcharenko", "Kravchenko", "Klymenko", "Panchenko", "Hrytsenko", "Holub", "Bondar", "Koval", "Tkach", "Kravets", "Shevchuk", "Mazur", "Fedorenko", "Sydorenko", "Romanenko", "Zakharchenko", "Dovhan", "Soroka", "Bilous", "Chumak", "Kozak", "Havryliuk", "Ivaniuk", "Tereshchenko", "Ostapenko", "Yakovenko", "Prykhodko", "Tymoshenko", "Lukianenko", "Vasylenko", "Nesterenko", "Demchenko", "Kostenko", "Symonenko", "Yaremchuk", "Hnatiuk", "Korniienko", "Samoilenko", "Danylchuk", "Mykhailenko", "Rybak", "Shapoval", "Kushnir", "Zozulia", "Bereza", "Lytvyn"];

// A small Biblical Hebrew-inspired patronymic grammar, rendered entirely in
// approachable Latin transliteration. It is a persona tradition, not a claim
// that one form covered every community or period of ancient Hebrew naming.
const ancientHebrewMaleNames = ["Aharon", "Avraham", "Avner", "Amos", "Asa", "Binyamin", "Boaz", "David", "Eli", "Eliyahu", "Elisha", "Eitan", "Ezra", "Gad", "Gavriel", "Gideon", "Hizqiyahu", "Hoshea", "Yitzhak", "Yishai", "Yaakov", "Yirmeyahu", "Yoel", "Yonah", "Yonatan", "Yosef", "Yehoshua", "Yehuda", "Kalev", "Levi", "Malakhi", "Menahem", "Mikhah", "Moshe", "Natan", "Nehemya", "Noah", "Ovadyah", "Reuven", "Shmuel", "Shaul", "Shimon", "Shlomo", "Uri", "Uriah", "Zekharyah", "Zephaniah", "Zuriel"];
const ancientHebrewFemaleNames = ["Avigayil", "Adah", "Atarah", "Bat-Sheva", "Bilhah", "Devorah", "Dinah", "Elisheva", "Esther", "Havah", "Hagar", "Hannah", "Huldah", "Yael", "Yemimah", "Leah", "Maakhah", "Mikhal", "Miriam", "Naamah", "Naomi", "Orpah", "Peninnah", "Rahel", "Rivqah", "Rut", "Sarah", "Serah", "Shiphrah", "Tamar", "Tirzah", "Tzipporah"];

// These are Amharic romanizations used as personal names in every position;
// the following elements are patronymics, never fabricated family surnames.
const amharicPersonalNames = ["Abay", "Abebe", "Addisu", "Alem", "Alemayehu", "Almaz", "Alula", "Amanuel", "Amare", "Asfaw", "Aster", "Azeb", "Bekele", "Berhane", "Betelhem", "Biniam", "Birhanu", "Birtukan", "Dawit", "Desta", "Eden", "Elias", "Ermias", "Eskinder", "Fasil", "Frehiwot", "Genet", "Getachew", "Girma", "Hana", "Haile", "Henok", "Hirut", "Kaleb", "Kebede", "Kidist", "Lemma", "Lulit", "Makeda", "Mekdes", "Melaku", "Meron", "Meseret", "Mulu", "Mulugeta", "Nahom", "Rahel", "Samuel", "Sara", "Selam", "Selamawit", "Senait", "Sileshi", "Solomon", "Tadesse", "Tewodros", "Tigist", "Tsehay", "Worku", "Yared", "Yonas", "Yosef", "Zenebech", "Zerihun"];

const portugueseMasculineGivenNames = ["Afonso", "Alexandre", "André", "António", "Bernardo", "Bruno", "Daniel", "Diogo", "Duarte", "Eduardo", "Francisco", "Gabriel", "Gonçalo", "Guilherme", "Henrique", "João", "Jorge", "Lourenço", "Luís", "Manuel", "Martim", "Miguel", "Nuno", "Pedro", "Rafael", "Ricardo", "Rodrigo", "Rui", "Salvador", "Santiago", "Simão", "Tiago", "Tomás", "Valentim", "Vasco"];
const portugueseFeminineGivenNames = ["Adriana", "Alice", "Amélia", "Ana", "Beatriz", "Benedita", "Camila", "Carolina", "Catarina", "Clara", "Diana", "Ema", "Francisca", "Helena", "Inês", "Joana", "Leonor", "Madalena", "Margarida", "Maria", "Mariana", "Marta", "Matilde", "Renata", "Rita", "Sara", "Sofia", "Teresa", "Vitória"];
const portugueseGivenNames = [...portugueseMasculineGivenNames, ...portugueseFeminineGivenNames];
const portugueseFamilyNames = ["Almeida", "Alves", "Amaral", "Andrade", "Araújo", "Azevedo", "Barbosa", "Barros", "Bastos", "Branco", "Brito", "Campos", "Cardoso", "Carvalho", "Castro", "Coelho", "Correia", "Costa", "Cruz", "Cunha", "Dias", "Duarte", "Esteves", "Faria", "Fernandes", "Ferreira", "Figueiredo", "Fonseca", "Freitas", "Gomes", "Gonçalves", "Guerreiro", "Henriques", "Leal", "Lima", "Lopes", "Loureiro", "Machado", "Marques", "Martins", "Matos", "Mendes", "Monteiro", "Morais", "Moreira", "Moura", "Neves", "Nogueira", "Nunes", "Oliveira", "Pacheco", "Paiva", "Pereira", "Pinto", "Pires", "Ramos", "Reis", "Ribeiro", "Rocha", "Rodrigues", "Santos", "Silva", "Soares", "Sousa"];

// Tamil initials deliberately stand as initials; the alternate form expands a
// selected patronymic without inventing a hereditary surname or caste title.
const tamilPatronymicInitials = ["A.", "B.", "C.", "D.", "E.", "G.", "H.", "I.", "J.", "K.", "L.", "M.", "N.", "P.", "R.", "S.", "T.", "U.", "V."];
const tamilPersonalNames = ["Aadhavan", "Abinaya", "Akilan", "Amudha", "Anand", "Anitha", "Aravind", "Arul", "Bala", "Bharathi", "Chandra", "Deepa", "Devan", "Dharani", "Divya", "Elango", "Gayathri", "Gokul", "Harini", "Ilamaran", "Indira", "Janani", "Jeeva", "Kannan", "Karthik", "Kavitha", "Kayal", "Keerthana", "Kumar", "Lakshmi", "Madhavan", "Malathi", "Meena", "Mohan", "Nandhini", "Nila", "Nirmal", "Pavithra", "Prakash", "Priya", "Raghavan", "Rajesh", "Revathi", "Sakthi", "Saranya", "Saravanan", "Selvi", "Senthil", "Shankar", "Sharmila", "Sivakumar", "Sowmya", "Subash", "Sundar", "Surya", "Tamilselvan", "Thamarai", "Thenmozhi", "Udhaya", "Valli", "Vasanth", "Vetri", "Vignesh", "Yazhini"];
const tamilParentNames = ["Annamalai", "Arumugam", "Balasubramanian", "Chidambaram", "Dhanapal", "Duraisamy", "Ganesan", "Govindan", "Ilango", "Jaganathan", "Kandasamy", "Kannappan", "Krishnan", "Kumaravel", "Mahalingam", "Manickam", "Marimuthu", "Mohanraj", "Murugan", "Muthu", "Nagarajan", "Natarajan", "Palanisamy", "Pandian", "Paramasivam", "Periyasamy", "Ponnusamy", "Prabhakaran", "Radhakrishnan", "Rajagopal", "Rajamanickam", "Rajaraman", "Ramasamy", "Ranganathan", "Ravichandran", "Sabapathy", "Saminathan", "Santhanam", "Sekar", "Selvaraj", "Shanmugam", "Sivagnanam", "Sivapalan", "Somasundaram", "Subramanian", "Sundaram", "Thangavel", "Thiagarajan", "Thirunavukarasu", "Ulaganathan", "Vadivel", "Varadarajan", "Vasudevan", "Velayutham", "Venkatesan", "Vijayaraghavan", "Viswanathan", "Yogeswaran", "Arulappan", "Devarajan", "Gopalakrishnan", "Kathirvel", "Loganathan", "Muthukumar"];

// Indonesian forms preserve the complete generated name. Following elements
// are not exposed or described as Western-style surnames.
const indonesianMononyms = ["Adi", "Agus", "Andi", "Arif", "Arum", "Ayu", "Bagus", "Bayu", "Bima", "Budi", "Cahya", "Citra", "Dedi", "Dewi", "Dian", "Dimas", "Eka", "Endah", "Fajar", "Farida", "Fitri", "Galih", "Gita", "Hadi", "Hana", "Indah", "Intan", "Joko", "Kartika", "Laras", "Lestari", "Made", "Maya", "Mega", "Nanda", "Nia", "Novi", "Nur", "Putra", "Putri", "Rahma", "Rama", "Rani", "Ratna", "Reza", "Rina", "Rizki", "Sari", "Sinta", "Sri", "Surya", "Taufik", "Teguh", "Tiara", "Tri", "Wati", "Widya", "Yani", "Yanto", "Yuda", "Yusuf", "Zahra", "Sekar", "Wulan"];
const indonesianFollowingNames = ["Adinata", "Ananda", "Anggraini", "Baskoro", "Batubara", "Cahyadi", "Daulay", "Dharmawan", "Firmansyah", "Ginting", "Gunawan", "Hakim", "Halim", "Handayani", "Harahap", "Hasibuan", "Hermawan", "Hidayat", "Hutapea", "Irawan", "Kencana", "Kurnia", "Kurniawan", "Kusnadi", "Kusuma", "Laksana", "Lim", "Lubis", "Lumbantobing", "Mahendra", "Maharani", "Maulana", "Mulyadi", "Nasution", "Ningsih", "Nugraha", "Pamungkas", "Permana", "Pertiwi", "Prakoso", "Pratama", "Purnama", "Rahayu", "Ramadhan", "Salim", "Santoso", "Saputra", "Sasmita", "Setiawan", "Simanjuntak", "Sinaga", "Siregar", "Situmorang", "Syahputra", "Syahrial", "Tan", "Tanuwijaya", "Tarigan", "Utami", "Wardana", "Wicaksono", "Wibowo", "Wijaya", "Winata"];

const welshMaleNames = ["Aled", "Aneirin", "Arwel", "Bedwyr", "Bleddyn", "Bran", "Bryn", "Cai", "Caradog", "Dafydd", "Dewi", "Dylan", "Elis", "Emrys", "Gareth", "Geraint", "Gethin", "Gruffudd", "Gwilym", "Harri", "Hedd", "Hywel", "Iestyn", "Ifan", "Ioan", "Iolo", "Llŷr", "Llywelyn", "Madoc", "Owain", "Rhodri", "Rhys"];
const welshFemaleNames = ["Angharad", "Anwen", "Arianwen", "Bethan", "Branwen", "Bronwen", "Carys", "Catrin", "Ceridwen", "Delyth", "Efa", "Eleri", "Elin", "Enid", "Ffion", "Gaenor", "Glenys", "Gwen", "Gwenllian", "Lowri", "Mair", "Manon", "Megan", "Meinir", "Nerys", "Non", "Olwen", "Rhiannon", "Seren", "Siân", "Sioned", "Tegwen"];
const welshFamilyNames = ["Anwyl", "Bebb", "Bevan", "Beynon", "Bowen", "Cadogan", "Cadwaladr", "Cadwallader", "Craddock", "Davies", "Edwards", "Ellis", "Evans", "Floyd", "Gethin", "Gough", "Griffith", "Griffiths", "Gwillim", "Gwyn", "Gwynne", "Harries", "Havard", "Hopkin", "Howell", "Hughes", "Humphreys", "James", "Jenkins", "John", "Jones", "Lewis", "Lloyd", "Maddocks", "Mathias", "Meredith", "Morgan", "Morris", "Mostyn", "Owen", "Owens", "Parry", "Penry", "Phillips", "Powell", "Prichard", "Price", "Pritchard", "Probert", "Prosser", "Rees", "Richards", "Roberts", "Rosser", "Rowlands", "Thomas", "Trevor", "Tudor", "Vaughan", "Walters", "Watkins", "Williams", "Wynne", "Yale"];

// Kurmanji is named explicitly because Kurdish languages use more than one
// script. These names retain the Latin Hawar letters ê, î, û, ç, and ş.
const kurmanjiPersonalNames = ["Agir", "Alan", "Aram", "Aras", "Arîn", "Avîn", "Azad", "Baran", "Bawer", "Berfin", "Berîvan", "Botan", "Çiya", "Ciwan", "Dara", "Delal", "Destan", "Dilan", "Dilovan", "Dilşad", "Evin", "Evîn", "Ferhad", "Goran", "Hêlîn", "Hejar", "Hogir", "Jînda", "Jiyan", "Kawa", "Kendal", "Koçer", "Kovan", "Lavîn", "Lorîn", "Mîran", "Mizgîn", "Nalin", "Newroz", "Nîgar", "Pîroz", "Rêber", "Rênas", "Rojan", "Rojbin", "Rojda", "Rojhat", "Rojîn", "Ronahî", "Serdar", "Serhat", "Sidar", "Sîpan", "Siyar", "Şervan", "Şilan", "Şîrîn", "Viyan", "Welat", "Xebat", "Zana", "Zelal", "Zinar", "Zozan"];
const kurmanjiFamilyOrLocativeNames = ["Amedî", "Barzanî", "Bedirxanî", "Behdînanî", "Berwarî", "Botanî", "Cizîrî", "Colemêrgî", "Dêrsimî", "Diyarbekirî", "Efrînî", "Erdelanî", "Garzanî", "Germiyanî", "Hekarî", "Hewlêrî", "Hewramî", "Kobanî", "Mêrdînî", "Mukriyanî", "Qamişloyî", "Rewandizî", "Serhedî", "Silêmanî", "Soranî", "Şengalî", "Urmiyê", "Wanî", "Xaneqînî", "Zagrosî", "Zaxoyî", "Zîlanî"];

const targusGivenNames = ["TARGUS"];
const targusFamilyNames = ["TARGUS"];

const elvishGivenOpenings = ["Ae", "Ael", "Aer", "Al", "An", "Ar", "Cael", "Cal", "El", "Ela", "Eli", "Fael", "Gal", "Ila", "Iri", "Lae", "Lio", "Mae", "Mira", "Nael", "Nim", "Ola", "Ori", "Rae", "Sael", "Syl", "Tha", "Thel", "Vael", "Yla", "Zae", "Zel"];
const elvishGivenEndings = ["driel", "fina", "ion", "ith", "lian", "lin", "lora", "mir", "myr", "na", "ndel", "ndra", "nel", "niel", "nor", "rien", "ril", "ris", "ron", "sari", "sel", "siel", "thas", "thel", "ther", "uil", "var", "viel", "wen", "wyn", "yra", "zor"];
const elvishHouseOpenings = ["Amber", "Ash", "Autumn", "Birch", "Bright", "Brook", "Cloud", "Dawn", "Dew", "Dream", "Dusk", "Ember", "Fern", "Frost", "Glen", "Golden", "Green", "Hollow", "Lark", "Leaf", "Moon", "Moss", "Night", "Oak", "Rain", "River", "Silver", "Star", "Sun", "Thorn", "Vale", "Willow"];
const elvishHouseEndings = ["bloom", "bough", "brook", "crown", "dancer", "dew", "dream", "fall", "fern", "fire", "glen", "glow", "grove", "heart", "hollow", "leaf", "light", "mere", "mist", "moon", "path", "rain", "river", "shade", "song", "star", "stone", "thorn", "vale", "ward", "wind", "wood"];

export const PERSONA_CATALOG_VERSION = 7;
const DOMAIN = "conduit-creche-persona-v1";
const MAX_DERIVATION_ATTEMPTS = 16;
const ROMANIZED_COMPONENT = /^[\p{Script=Latin}\p{Mark} .'-]+$/u;

const systems = Object.freeze([
  namingSystem("roman", "Roman tria nomina", [
    stock("roman_praenomina", romanPraenomina), stock("roman_nomina", romanNomina), stock("roman_cognomina", romanCognomina),
  ], [nameForm("roman_tria_nomina", ["roman_praenomina", "roman_nomina", "roman_cognomina"], (value) => `${value.roman_praenomina} ${value.roman_nomina} ${value.roman_cognomina}`)]),
  namingSystem("chinese", "Chinese (romanized, family name first)", [
    stock("chinese_family", chineseFamilyNames), stock("chinese_given", chineseGivenNames),
  ], [nameForm("chinese_family_given", ["chinese_family", "chinese_given"], (value) => `${value.chinese_family} ${value.chinese_given}`)]),
  namingSystem("american", "Contemporary American", [
    stock("american_given", americanGivenNames), stock("american_middle", americanMiddleNames), stock("american_family", americanFamilyNames),
  ], [
    nameForm("american_given_family", ["american_given", "american_family"], (value) => `${value.american_given} ${value.american_family}`),
    nameForm("american_given_middle_family", ["american_given", "american_middle", "american_family"], (value) => `${value.american_given} ${value.american_middle} ${value.american_family}`),
  ]),
  namingSystem("mexican", "Mexican (two surnames)", [
    stock("mexican_given", mexicanGivenNames), stock("mexican_masculine_given", mexicanMasculineGivenNames), stock("mexican_feminine_given", mexicanFeminineGivenNames), stock("mexican_masculine_additional", mexicanMasculineAdditionalNames), stock("mexican_feminine_additional", mexicanFeminineAdditionalNames), stock("mexican_first_surname", mexicanFamilyNames), stock("mexican_second_surname", mexicanFamilyNames),
  ], [
    nameForm("mexican_given_two_surnames", ["mexican_given", "mexican_first_surname", "mexican_second_surname"], (value) => `${value.mexican_given} ${value.mexican_first_surname} ${value.mexican_second_surname}`),
    nameForm("mexican_masculine_two_given_two_surnames", ["mexican_masculine_given", "mexican_masculine_additional", "mexican_first_surname", "mexican_second_surname"], (value) => `${value.mexican_masculine_given} ${value.mexican_masculine_additional} ${value.mexican_first_surname} ${value.mexican_second_surname}`),
    nameForm("mexican_feminine_two_given_two_surnames", ["mexican_feminine_given", "mexican_feminine_additional", "mexican_first_surname", "mexican_second_surname"], (value) => `${value.mexican_feminine_given} ${value.mexican_feminine_additional} ${value.mexican_first_surname} ${value.mexican_second_surname}`),
  ]),
  namingSystem("icelandic", "Icelandic patronymic or matronymic", [
    stock("icelandic_given", icelandicGivenNames), stock("icelandic_parent_genitive", icelandicParentGenitives), stock("icelandic_ending", icelandicEndings),
  ], [nameForm("icelandic_parent_name", ["icelandic_given", "icelandic_parent_genitive", "icelandic_ending"], (value) => `${value.icelandic_given} ${value.icelandic_parent_genitive}${value.icelandic_ending}`)]),
  namingSystem("japanese", "Japanese (romanized, family name first)", [
    stock("japanese_family", japaneseFamilyNames), stock("japanese_given", japaneseGivenNames),
  ], [nameForm("japanese_family_given", ["japanese_family", "japanese_given"], (value) => `${value.japanese_family} ${value.japanese_given}`)]),
  namingSystem("arabic", "Arabic lineage form", [
    stock("arabic_given_lineage", arabicGivenLineages), stock("arabic_parent", arabicParentNames), stock("arabic_family", arabicFamilyNames),
  ], [nameForm("arabic_given_lineage_family", ["arabic_given_lineage", "arabic_parent", "arabic_family"], (value) => `${value.arabic_given_lineage} ${value.arabic_parent} ${value.arabic_family}`)]),
  namingSystem("french", "Contemporary French", [
    stock("french_given", frenchGivenNames), stock("french_compound_given", frenchCompoundGivenNames), stock("french_first_family", frenchFamilyNames), stock("french_second_family", frenchFamilyNames),
  ], [
    nameForm("french_given_family", ["french_given", "french_first_family"], (value) => `${value.french_given} ${value.french_first_family}`),
    nameForm("french_compound_given_family", ["french_compound_given", "french_first_family"], (value) => `${value.french_compound_given} ${value.french_first_family}`),
    nameForm("french_given_double_family", ["french_given", "french_first_family", "french_second_family"], (value) => `${value.french_given} ${value.french_first_family} ${value.french_second_family}`),
  ]),
  namingSystem("british", "Contemporary British", [
    stock("british_given", britishGivenNames), stock("british_masculine_given", britishMasculineGivenNames), stock("british_feminine_given", britishFeminineGivenNames), stock("british_masculine_middle", britishMasculineMiddleNames), stock("british_feminine_middle", britishFeminineMiddleNames), stock("british_first_family", britishFamilyNames), stock("british_second_family", britishFamilyNames),
  ], [
    nameForm("british_given_family", ["british_given", "british_first_family"], (value) => `${value.british_given} ${value.british_first_family}`),
    nameForm("british_masculine_given_middle_family", ["british_masculine_given", "british_masculine_middle", "british_first_family"], (value) => `${value.british_masculine_given} ${value.british_masculine_middle} ${value.british_first_family}`),
    nameForm("british_feminine_given_middle_family", ["british_feminine_given", "british_feminine_middle", "british_first_family"], (value) => `${value.british_feminine_given} ${value.british_feminine_middle} ${value.british_first_family}`),
    nameForm("british_double_barrelled", ["british_given", "british_first_family", "british_second_family"], (value) => `${value.british_given} ${value.british_first_family}-${value.british_second_family}`),
  ]),
  namingSystem("classic-anglophone", "Classic Anglophone", [
    stock("classic_anglophone_given", classicAnglophoneGivenNames), stock("classic_anglophone_masculine_given", classicAnglophoneMasculineGivenNames), stock("classic_anglophone_feminine_given", classicAnglophoneFeminineGivenNames), stock("classic_anglophone_masculine_middle", classicAnglophoneMasculineMiddleNames), stock("classic_anglophone_feminine_middle", classicAnglophoneFeminineMiddleNames), stock("classic_anglophone_family", classicAnglophoneFamilyNames),
  ], [
    nameForm("classic_anglophone_given_family", ["classic_anglophone_given", "classic_anglophone_family"], (value) => `${value.classic_anglophone_given} ${value.classic_anglophone_family}`),
    nameForm("classic_anglophone_masculine_given_middle_family", ["classic_anglophone_masculine_given", "classic_anglophone_masculine_middle", "classic_anglophone_family"], (value) => `${value.classic_anglophone_masculine_given} ${value.classic_anglophone_masculine_middle} ${value.classic_anglophone_family}`),
    nameForm("classic_anglophone_feminine_given_middle_family", ["classic_anglophone_feminine_given", "classic_anglophone_feminine_middle", "classic_anglophone_family"], (value) => `${value.classic_anglophone_feminine_given} ${value.classic_anglophone_feminine_middle} ${value.classic_anglophone_family}`),
  ]),
  namingSystem("korean", "Korean (romanized, family name first)", [
    stock("korean_family", koreanFamilyNames), stock("korean_given", koreanGivenNames),
  ], [nameForm("korean_family_given", ["korean_family", "korean_given"], (value) => `${value.korean_family} ${value.korean_given}`)]),
  namingSystem("vietnamese", "Vietnamese (family–middle–given)", [
    stock("vietnamese_family", vietnameseFamilyNames), stock("vietnamese_middle", vietnameseMiddleNames), stock("vietnamese_given", vietnameseGivenNames),
  ], [nameForm("vietnamese_family_middle_given", ["vietnamese_family", "vietnamese_middle", "vietnamese_given"], (value) => `${value.vietnamese_family} ${value.vietnamese_middle} ${value.vietnamese_given}`)]),
  namingSystem("yoruba", "Yorùbá personal and family names", [
    stock("yoruba_personal", yorubaPersonalNames), stock("yoruba_family", yorubaFamilyNames),
  ], [nameForm("yoruba_personal_family", ["yoruba_personal", "yoruba_family"], (value) => `${value.yoruba_personal} ${value.yoruba_family}`)]),
  namingSystem("ukrainian", "Ukrainian (official romanization)", [
    stock("ukrainian_given", ukrainianGivenNames), stock("ukrainian_family", ukrainianFamilyNames),
  ], [nameForm("ukrainian_given_family", ["ukrainian_given", "ukrainian_family"], (value) => `${value.ukrainian_given} ${value.ukrainian_family}`)]),
  namingSystem("ancient-hebrew", "Biblical Hebrew-inspired patronymic (romanized)", [
    stock("ancient_hebrew_male", ancientHebrewMaleNames), stock("ancient_hebrew_female", ancientHebrewFemaleNames), stock("ancient_hebrew_ancestor", ancientHebrewMaleNames),
  ], [
    nameForm("ancient_hebrew_ben", ["ancient_hebrew_male", "ancient_hebrew_ancestor"], (value) => `${value.ancient_hebrew_male} ben ${value.ancient_hebrew_ancestor}`),
    nameForm("ancient_hebrew_bat", ["ancient_hebrew_female", "ancient_hebrew_ancestor"], (value) => `${value.ancient_hebrew_female} bat ${value.ancient_hebrew_ancestor}`),
  ]),
  namingSystem("amharic", "Amharic-style patronymic (romanized)", [
    stock("amharic_personal", amharicPersonalNames), stock("amharic_parent", amharicPersonalNames), stock("amharic_grandparent", amharicPersonalNames),
  ], [
    nameForm("amharic_personal_parent", ["amharic_personal", "amharic_parent"], (value) => `${value.amharic_personal} ${value.amharic_parent}`),
    nameForm("amharic_personal_parent_grandparent", ["amharic_personal", "amharic_parent", "amharic_grandparent"], (value) => `${value.amharic_personal} ${value.amharic_parent} ${value.amharic_grandparent}`),
  ]),
  namingSystem("portuguese", "Portuguese multi-surname", [
    stock("portuguese_first_given", portugueseGivenNames), stock("portuguese_masculine_first_given", portugueseMasculineGivenNames), stock("portuguese_feminine_first_given", portugueseFeminineGivenNames), stock("portuguese_masculine_second_given", portugueseMasculineGivenNames), stock("portuguese_feminine_second_given", portugueseFeminineGivenNames), stock("portuguese_first_family", portugueseFamilyNames), stock("portuguese_second_family", portugueseFamilyNames),
  ], [
    nameForm("portuguese_given_two_family", ["portuguese_first_given", "portuguese_first_family", "portuguese_second_family"], (value) => `${value.portuguese_first_given} ${value.portuguese_first_family} ${value.portuguese_second_family}`),
    nameForm("portuguese_masculine_two_given_two_family", ["portuguese_masculine_first_given", "portuguese_masculine_second_given", "portuguese_first_family", "portuguese_second_family"], (value) => `${value.portuguese_masculine_first_given} ${value.portuguese_masculine_second_given} ${value.portuguese_first_family} ${value.portuguese_second_family}`),
    nameForm("portuguese_feminine_two_given_two_family", ["portuguese_feminine_first_given", "portuguese_feminine_second_given", "portuguese_first_family", "portuguese_second_family"], (value) => `${value.portuguese_feminine_first_given} ${value.portuguese_feminine_second_given} ${value.portuguese_first_family} ${value.portuguese_second_family}`),
  ]),
  namingSystem("tamil", "Tamil patronymic forms (romanized)", [
    stock("tamil_patronymic_initial", tamilPatronymicInitials), stock("tamil_personal", tamilPersonalNames), stock("tamil_parent", tamilParentNames),
  ], [
    nameForm("tamil_initial_personal", ["tamil_patronymic_initial", "tamil_personal"], (value) => `${value.tamil_patronymic_initial} ${value.tamil_personal}`),
    nameForm("tamil_personal_parent", ["tamil_personal", "tamil_parent"], (value) => `${value.tamil_personal} ${value.tamil_parent}`),
  ]),
  namingSystem("indonesian", "Indonesian complete personal-name forms (romanized)", [
    stock("indonesian_mononym", indonesianMononyms), stock("indonesian_personal", indonesianMononyms), stock("indonesian_following", indonesianFollowingNames),
  ], [
    nameForm("indonesian_mononym", ["indonesian_mononym"], (value) => value.indonesian_mononym),
    nameForm("indonesian_two_part", ["indonesian_personal", "indonesian_following"], (value) => `${value.indonesian_personal} ${value.indonesian_following}`),
  ]),
  namingSystem("welsh", "Welsh modern and patronymic forms", [
    stock("welsh_male", welshMaleNames), stock("welsh_female", welshFemaleNames), stock("welsh_parent", welshMaleNames), stock("welsh_family", welshFamilyNames),
  ], [
    nameForm("welsh_male_family", ["welsh_male", "welsh_family"], (value) => `${value.welsh_male} ${value.welsh_family}`),
    nameForm("welsh_female_family", ["welsh_female", "welsh_family"], (value) => `${value.welsh_female} ${value.welsh_family}`),
    nameForm("welsh_ap_patronymic", ["welsh_male", "welsh_parent"], (value) => `${value.welsh_male} ap ${value.welsh_parent}`),
    nameForm("welsh_ferch_patronymic", ["welsh_female", "welsh_parent"], (value) => `${value.welsh_female} ferch ${value.welsh_parent}`),
  ]),
  namingSystem("kurmanji", "Kurdish Kurmanji (Latin script)", [
    stock("kurmanji_personal", kurmanjiPersonalNames), stock("kurmanji_family_or_locative", kurmanjiFamilyOrLocativeNames),
  ], [nameForm("kurmanji_personal_family_or_locative", ["kurmanji_personal", "kurmanji_family_or_locative"], (value) => `${value.kurmanji_personal} ${value.kurmanji_family_or_locative}`)]),
  namingSystem("targus", "The TARGUS family", [
    stock("targus_given", targusGivenNames), stock("targus_family", targusFamilyNames),
  ], [nameForm("targus_given_family", ["targus_given", "targus_family"], (value) => `${value.targus_given} ${value.targus_family}`)]),
  namingSystem("elvish", "Original Elvish-inspired", [
    stock("elvish_given_opening", elvishGivenOpenings), stock("elvish_given_ending", elvishGivenEndings), stock("elvish_house_opening", elvishHouseOpenings), stock("elvish_house_ending", elvishHouseEndings),
  ], [nameForm("elvish_given_house", ["elvish_given_opening", "elvish_given_ending", "elvish_house_opening", "elvish_house_ending"], (value) => `${value.elvish_given_opening}${value.elvish_given_ending} ${value.elvish_house_opening}${value.elvish_house_ending}`)]),
]);

export const PERSONA_SYSTEMS = systems;

export const NAMING_SYSTEM_OPTIONS = Object.freeze([
  Object.freeze({ id: "surprise", label: "Surprise me" }),
  ...systems.map(({ id, label }) => Object.freeze({ id, label })),
]);

export const PERSONA_SYSTEM_COUNTS = Object.freeze(Object.fromEntries(systems.map(({ id, size }) => [id, size])));
export const NAME_NAMESPACE_SIZE = systems.reduce((total, entry) => total + entry.size, 0n);

export async function nameFor(uuid, requestedSystem = "surprise", variation = 0, cryptoProvider = globalThis.crypto) {
  const normalizedUuid = normalizeUuid(uuid);
  if (requestedSystem !== "surprise" && !systems.some(({ id }) => id === requestedSystem)) {
    throw new TypeError(`unknown persona naming system ${requestedSystem}`);
  }
  if (!Number.isSafeInteger(variation) || variation < 0 || variation > 0xffff_ffff) {
    throw new TypeError("persona variation must be an unsigned 32-bit integer");
  }
  if (!cryptoProvider?.subtle || typeof cryptoProvider.subtle.digest !== "function") {
    throw new TypeError("persona derivation requires a SHA-256 provider");
  }
  for (let counter = 0; counter < MAX_DERIVATION_ATTEMPTS; counter += 1) {
    const entropy = new Uint8Array(await cryptoProvider.subtle.digest(
      "SHA-256",
      encoder.encode(JSON.stringify([DOMAIN, PERSONA_CATALOG_VERSION, normalizedUuid, requestedSystem, variation, counter])),
    ));
    const generated = selectPersona(entropy, requestedSystem, variation);
    if (encoder.encode(generated.name).length <= MAX_FRIENDLY_NAME_BYTES) return generated;
  }
  throw new RangeError("persona derivation could not satisfy the friendly-name UTF-8 bound");
}

function selectPersona(entropy, requestedSystem, variation) {
  let word = 0;
  const nextIndex = (length) => {
    const offset = word++ * 4;
    return new DataView(entropy.buffer, entropy.byteOffset + offset, 4).getUint32(0, false) % length;
  };
  const selectedSystem = requestedSystem === "surprise" ? systems[nextIndex(systems.length)] : systems.find(({ id }) => id === requestedSystem);
  let formTicket = BigInt(nextIndex(Number(selectedSystem.size)));
  const selectedForm = selectedSystem.forms.find((form) => {
    if (formTicket < form.size) return true;
    formTicket -= form.size;
    return false;
  });
  const values = {};
  const stockIndexes = selectedForm.slots.map((stockId) => {
    const selectedStock = selectedSystem.stocks.find(({ id }) => id === stockId);
    const index = nextIndex(selectedStock.entries.length);
    values[stockId] = selectedStock.entries[index];
    return Object.freeze({ stock_id: stockId, index });
  });
  return Object.freeze({
    name: selectedForm.assemble(values),
    version: PERSONA_CATALOG_VERSION,
    system_id: selectedSystem.id,
    system_label: selectedSystem.label,
    form_id: selectedForm.id,
    variation,
    stock_indexes: Object.freeze(stockIndexes),
  });
}

function stock(id, entries) {
  if (!/^[a-z][a-z0-9_]*$/.test(id)
    || entries.length === 0
    || entries.some((entry) => typeof entry !== "string" || entry.length === 0 || !ROMANIZED_COMPONENT.test(entry))
    || new Set(entries).size !== entries.length) {
    throw new TypeError(`invalid persona stock ${id}`);
  }
  return Object.freeze({ id, entries: Object.freeze(entries) });
}

function nameForm(id, slots, assemble) {
  return Object.freeze({ id, slots: Object.freeze(slots), assemble });
}

function namingSystem(id, label, stocks, forms) {
  const stockIds = new Set(stocks.map((entry) => entry.id));
  if (stockIds.size !== stocks.length || forms.some((form) => form.slots.some((slot) => !stockIds.has(slot)))) {
    throw new TypeError(`invalid persona naming system ${id}`);
  }
  const sizedForms = forms.map((form) => Object.freeze({ ...form, size: form.slots.reduce((count, slot) => {
    const selectedStock = stocks.find(({ id: stockId }) => stockId === slot);
    return count * BigInt(selectedStock.entries.length);
  }, 1n) }));
  const size = sizedForms.reduce((total, form) => total + form.size, 0n);
  if (size > 0xffff_ffffn) throw new TypeError(`persona naming system ${id} exceeds its selection bound`);
  return Object.freeze({ id, label, version: PERSONA_CATALOG_VERSION, stocks: Object.freeze(stocks), forms: Object.freeze(sizedForms), size });
}

function normalizeUuid(value) {
  if (typeof value !== "string" || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value)) {
    throw new TypeError("persona derivation requires a canonical UUID string");
  }
  return value.toLowerCase();
}
