using Avalonia.Controls;
using LogiTux.App.ViewModels;

namespace LogiTux.App.Views;

public partial class MainWindow : Window
{
    private readonly MainWindowViewModel _vm = new();

    public MainWindow()
    {
        InitializeComponent();
        DataContext = _vm;
        Closed += (_, _) => _vm.DisposeAsync().AsTask().GetAwaiter().GetResult();
    }
}