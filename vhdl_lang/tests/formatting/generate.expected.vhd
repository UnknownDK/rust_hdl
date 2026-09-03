architecture rtl of foo is
begin
    gen: for i in 0 to 1 generate
        inst: entity work.child
            generic map (
                index => i,
                width => 8
            )
            port map (
                clk => clk,
                data => data(i)
            );
    end generate;
end architecture;
