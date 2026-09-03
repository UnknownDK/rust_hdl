entity foo is
    port (
        a: in std_logic;
        b: out std_logic
    );
end entity;

architecture rtl of foo is
    signal x: std_logic;
    signal y: std_logic;
begin
    process(x)
    begin
        if x = '1' then
            y <= x;
        else
            y <= '0';
        end if;
    end process;
end architecture;
